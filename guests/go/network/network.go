// Package network connects TinyGo's net package to WASI sockets.
// Enable must be called explicitly. Importing this package creates no host resources.
package network

import (
	"errors"
	"io"
	"math"
	"net"
	"net/netip"
	"sync"
	"time"
)

// ErrUnsupported identifies operations not provided by this adapter, including
// UDP, listening sockets, TLS and native Go installation.
var ErrUnsupported = errors.New("network: operation not supported")

// Options configures the guest driver, without changing host permissions.
type Options struct {
	// Timeout bounds DNS resolution and TCP connection establishment. Zero uses
	// five seconds. TinyGo's netdev API does not pass net.Dialer deadlines or contexts.
	Timeout time.Duration
}

type backend interface {
	lookup(string, time.Time) (netip.Addr, error)
	socket(int) (connection, error)
}

type connection interface {
	connect(netip.AddrPort, time.Time) error
	read([]byte, time.Time) (int, error)
	write([]byte, time.Time) (int, error)
	close() error
	option(int, int, any) error
}

// device implements the interface expected by TinyGo 0.43's net.useNetdev.
// Handles are private integers; their host ownership remains in connection.
type device struct {
	mu          sync.Mutex
	backend     backend
	timeout     time.Duration
	next        int
	connections map[int]connection
}

func newDevice(b backend, options Options) (*device, error) {
	if options.Timeout < 0 {
		return nil, errors.New("network: negative timeout")
	}
	if options.Timeout == 0 {
		options.Timeout = 5 * time.Second
	}
	return &device{backend: b, timeout: options.Timeout, connections: make(map[int]connection)}, nil
}

func (d *device) GetHostByName(name string) (netip.Addr, error) {
	if ip, err := netip.ParseAddr(name); err == nil {
		if ip.Zone() != "" {
			return netip.Addr{}, ErrUnsupported
		}
		return ip.Unmap(), nil
	}
	return d.backend.lookup(name, time.Now().Add(d.timeout))
}
func (d *device) Addr() (netip.Addr, error) { return netip.Addr{}, ErrUnsupported }
func (d *device) Socket(family, kind, protocol int) (int, error) {
	if (family != 2 && family != 10) || kind != 1 || (protocol != 0 && protocol != 6) {
		return -1, ErrUnsupported
	}
	c, err := d.backend.socket(family)
	if err != nil {
		return -1, err
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	d.next++
	d.connections[d.next] = c
	return d.next, nil
}
func (d *device) get(fd int) (connection, error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	c := d.connections[fd]
	if c == nil {
		return nil, net.ErrClosed
	}
	return c, nil
}
func (d *device) Connect(fd int, _ string, addr netip.AddrPort) error {
	c, err := d.get(fd)
	if err != nil {
		return err
	}
	return c.connect(addr, time.Now().Add(d.timeout))
}
func (d *device) Send(fd int, b []byte, flags int, deadline time.Time) (int, error) {
	if flags != 0 {
		return 0, ErrUnsupported
	}
	c, err := d.get(fd)
	if err != nil {
		return 0, err
	}
	return c.write(b, deadline)
}
func (d *device) Recv(fd int, b []byte, flags int, deadline time.Time) (int, error) {
	if flags != 0 {
		return 0, ErrUnsupported
	}
	c, err := d.get(fd)
	if err != nil {
		return 0, err
	}
	return c.read(b, deadline)
}
func (d *device) Close(fd int) error {
	d.mu.Lock()
	c := d.connections[fd]
	delete(d.connections, fd)
	d.mu.Unlock()
	if c == nil {
		return nil
	}
	return c.close()
}
func (d *device) SetSockOpt(fd, level, option int, value any) error {
	c, err := d.get(fd)
	if err != nil {
		return err
	}
	return c.option(level, option, value)
}
func (d *device) Bind(int, netip.AddrPort) error { return ErrUnsupported }
func (d *device) Listen(int, int) error          { return ErrUnsupported }
func (d *device) Accept(int) (int, netip.AddrPort, error) {
	return -1, netip.AddrPort{}, ErrUnsupported
}

// TinyGo passes TCPConn.SetKeepAlivePeriod as float64 in half-second units.
func keepAlivePeriod(value any) (time.Duration, error) {
	v, ok := value.(float64)
	if !ok || v <= 0 || math.IsNaN(v) || math.IsInf(v, 0) ||
		v >= float64(math.MaxInt64)/float64(time.Second/2) {
		return 0, ErrUnsupported
	}
	return time.Duration(v * float64(time.Second/2)), nil
}

// outputStream separates the WASI write/flush protocol from ABI and ownership.
// Each operation must protect its resource against concurrent Close.
type outputStream interface {
	checkWrite(time.Time) (uint64, error)
	write([]byte, time.Time) error
	flush(time.Time) error
	wait(time.Time) error
}

func writeStream(out outputStream, b []byte, deadline time.Time) (int, error) {
	written := 0
	for written < len(b) {
		permit, err := out.checkWrite(deadline)
		if err != nil {
			return written, writeError(err)
		}
		n := int(min(permit, uint64(len(b)-written)))
		if n != 0 {
			if err := out.write(b[written:written+n], deadline); err != nil {
				return written, writeError(err)
			}
			written += n
			if err := out.flush(deadline); err != nil {
				return written, writeError(err)
			}
		}
		if err := out.wait(deadline); err != nil {
			return written, err
		}
	}
	// Readiness can signal a failed asynchronous flush. Check its result even
	// after the final chunk; otherwise Write would incorrectly report success.
	if written != 0 {
		_, err := out.checkWrite(deadline)
		return written, writeError(err)
	}
	return 0, nil
}

func writeError(err error) error {
	if errors.Is(err, io.EOF) {
		return io.ErrClosedPipe
	}
	return err
}

// pause lets TinyGo's asyncify scheduler run HTTP reader/writer goroutines.
// Blocking an entire component in poll.block would deadlock that transport.
func pause(deadline time.Time) error {
	interval := time.Millisecond
	if !deadline.IsZero() {
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return &timeoutError{}
		}
		if remaining < interval {
			interval = remaining
		}
	}
	time.Sleep(interval)
	return nil
}

type timeoutError struct{}

func (*timeoutError) Error() string   { return "network: i/o timeout" }
func (*timeoutError) Timeout() bool   { return true }
func (*timeoutError) Temporary() bool { return true }
