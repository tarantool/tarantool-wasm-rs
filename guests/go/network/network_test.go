package network

import (
	"errors"
	"io"
	"math"
	"net"
	"net/netip"
	"sync"
	"testing"
	"time"
)

type fakeBackend struct {
	lookups     int
	connections []*fakeConnection
	err         error
	deadline    time.Time
}

func (b *fakeBackend) lookup(_ string, deadline time.Time) (netip.Addr, error) {
	b.lookups++
	b.deadline = deadline
	return netip.MustParseAddr("127.0.0.1"), b.err
}
func (b *fakeBackend) socket(int) (connection, error) {
	if b.err != nil {
		return nil, b.err
	}
	c := &fakeConnection{}
	b.connections = append(b.connections, c)
	return c, nil
}

type fakeConnection struct {
	mu       sync.Mutex
	closed   bool
	closes   int
	deadline time.Time
}

func (c *fakeConnection) connect(_ netip.AddrPort, deadline time.Time) error {
	c.deadline = deadline
	return nil
}
func (c *fakeConnection) read(b []byte, _ time.Time) (int, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.closed {
		return 0, net.ErrClosed
	}
	return copy(b, "ok"), nil
}
func (c *fakeConnection) write(b []byte, _ time.Time) (int, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.closed {
		return 0, net.ErrClosed
	}
	return len(b), nil
}
func (c *fakeConnection) close() error {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.closed = true
	c.closes++
	return nil
}
func (*fakeConnection) option(int, int, any) error { return nil }

func TestLiteralAddressesDoNotResolve(t *testing.T) {
	b := &fakeBackend{}
	d, err := newDevice(b, Options{})
	if err != nil {
		t.Fatal(err)
	}
	for _, text := range []string{"127.0.0.1", "::1", "::ffff:127.0.0.1"} {
		ip, err := d.GetHostByName(text)
		if err != nil || ip != netip.MustParseAddr(text).Unmap() {
			t.Fatalf("%s: %v, %v", text, ip, err)
		}
	}
	if b.lookups != 0 {
		t.Fatal("numeric addresses made a host DNS call")
	}
	if _, err := d.GetHostByName("fe80::1%lo"); !errors.Is(err, ErrUnsupported) {
		t.Fatal(err)
	}
}

func TestDescriptorOwnership(t *testing.T) {
	b := &fakeBackend{}
	d, err := newDevice(b, Options{Timeout: time.Second})
	if err != nil {
		t.Fatal(err)
	}
	first, err := d.Socket(2, 1, 6)
	if err != nil {
		t.Fatal(err)
	}
	second, err := d.Socket(10, 1, 6)
	if err != nil {
		t.Fatal(err)
	}
	if first == second {
		t.Fatal("two sockets share a descriptor")
	}
	if err := d.Connect(first, "", netip.MustParseAddrPort("127.0.0.1:80")); err != nil {
		t.Fatal(err)
	}
	if left := time.Until(b.connections[0].deadline); left <= 0 || left > time.Second {
		t.Fatal(left)
	}
	if _, err := d.Recv(first, make([]byte, 2), 0, time.Time{}); err != nil {
		t.Fatal(err)
	}
	var group sync.WaitGroup
	for i := 0; i < 8; i++ {
		group.Add(1)
		go func() {
			defer group.Done()
			if err := d.Close(first); err != nil {
				t.Error(err)
			}
		}()
	}
	group.Wait()
	if b.connections[0].closes != 1 {
		t.Fatal("close must release exactly one owner")
	}
	if _, err := d.Send(first, []byte("x"), 0, time.Time{}); !errors.Is(err, net.ErrClosed) {
		t.Fatal(err)
	}
	if _, err := d.Send(second, []byte("x"), 0, time.Time{}); err != nil {
		t.Fatal(err)
	}
	if err := d.Close(second); err != nil {
		t.Fatal(err)
	}
	if len(d.connections) != 0 {
		t.Fatal("closed descriptors retained")
	}
}

func TestErrorsAndTimeouts(t *testing.T) {
	failure := errors.New("denied")
	b := &fakeBackend{err: failure}
	d, err := newDevice(b, Options{Timeout: time.Second})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := d.Socket(2, 1, 6); !errors.Is(err, failure) {
		t.Fatal(err)
	}
	if len(d.connections) != 0 {
		t.Fatal("failed socket leaked a descriptor")
	}
	if _, err := d.GetHostByName("localhost"); !errors.Is(err, failure) {
		t.Fatal(err)
	}
	if left := time.Until(b.deadline); left <= 0 || left > time.Second {
		t.Fatal(left)
	}
	for _, protocol := range []int{17, 254} {
		if _, err := d.Socket(2, 1, protocol); !errors.Is(err, ErrUnsupported) {
			t.Fatal(err)
		}
	}
	if _, err := newDevice(b, Options{Timeout: -1}); err == nil {
		t.Fatal("negative timeout accepted")
	}
	err = pause(time.Now().Add(-time.Second))
	var networkError net.Error
	if !errors.As(err, &networkError) || !networkError.Timeout() {
		t.Fatal(err)
	}
}

func TestKeepAlivePeriod(t *testing.T) {
	for _, period := range []time.Duration{time.Second / 2, 1500 * time.Millisecond, time.Minute} {
		got, err := keepAlivePeriod(2 * period.Seconds())
		if err != nil || got != period {
			t.Fatalf("period %v: %v, %v", period, got, err)
		}
	}
	for _, value := range []any{1, true, float64(0), float64(-1), math.Inf(1), math.NaN(), math.MaxFloat64} {
		if _, err := keepAlivePeriod(value); !errors.Is(err, ErrUnsupported) {
			t.Fatalf("%v: %v", value, err)
		}
	}
}

type fakeOutput struct {
	permits               []uint64
	chunks                []string
	flushed, completed    bool
	checks, waits         int
	flushFailure, waitErr error
}

func (s *fakeOutput) checkWrite(time.Time) (uint64, error) {
	s.checks++
	if s.completed && s.flushFailure != nil {
		return 0, s.flushFailure
	}
	if len(s.permits) == 0 {
		return 2, nil
	}
	n := s.permits[0]
	s.permits = s.permits[1:]
	return n, nil
}
func (s *fakeOutput) write(b []byte, _ time.Time) error {
	s.chunks = append(s.chunks, string(b))
	s.flushed, s.completed = false, false
	return nil
}
func (s *fakeOutput) flush(time.Time) error {
	s.flushed = true
	return nil // Failure is asynchronous and only observable after readiness.
}
func (s *fakeOutput) wait(time.Time) error {
	s.waits++
	s.completed = s.flushed
	return s.waitErr
}

func TestFinalFlushError(t *testing.T) {
	failure := errors.New("asynchronous final flush failure")
	s := &fakeOutput{flushFailure: failure}
	n, err := writeStream(s, []byte("x"), time.Time{})
	if n != 1 || !errors.Is(err, failure) || s.checks != 2 || s.waits != 1 {
		t.Fatalf("final flush result lost: n=%d err=%v checks=%d waits=%d", n, err, s.checks, s.waits)
	}
	s = &fakeOutput{flushFailure: io.EOF}
	if _, err := writeStream(s, []byte("x"), time.Time{}); !errors.Is(err, io.ErrClosedPipe) {
		t.Fatal(err)
	}
}

func TestWriteBackpressure(t *testing.T) {
	s := &fakeOutput{permits: []uint64{0, 2, 2, 2}}
	n, err := writeStream(s, []byte("hello"), time.Time{})
	if err != nil || n != 5 || len(s.chunks) != 3 || s.chunks[0] != "he" ||
		s.chunks[1] != "ll" || s.chunks[2] != "o" || s.waits != 4 {
		t.Fatalf("partial writes: n=%d err=%v chunks=%v waits=%d", n, err, s.chunks, s.waits)
	}
	s = &fakeOutput{waitErr: &timeoutError{}}
	n, err = writeStream(s, []byte("hello"), time.Time{})
	var timeout interface{ Timeout() bool }
	if n != 2 || !errors.As(err, &timeout) || !timeout.Timeout() {
		t.Fatalf("partial count lost on timeout: n=%d err=%v", n, err)
	}
}
