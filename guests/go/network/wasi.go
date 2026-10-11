//go:build tinygo.wasm && wasip2

package network

import (
	"encoding/binary"
	"errors"
	"io"
	"net"
	"net/netip"
	"os"
	"sync"
	"time"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/io/v0.2.12/poll"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/io/v0.2.12/streams"
	instancenetwork "github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/sockets/v0.2.12/instance-network"
	ipnamelookup "github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/sockets/v0.2.12/ip-name-lookup"
	wnet "github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/sockets/v0.2.12/network"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/sockets/v0.2.12/tcp"
	tcpcreatesocket "github.com/tarantool/tarantool-wasm-rs/guests/go/network/internal/wasi/sockets/v0.2.12/tcp-create-socket"
	"go.bytecodealliance.org/cm"
)

type wasiBackend struct{}

func (wasiBackend) lookup(name string, deadline time.Time) (netip.Addr, error) {
	network := instancenetwork.InstanceNetwork()
	defer network.ResourceDrop()
	r := ipnamelookup.ResolveAddresses(network, name)
	if err := socketError(r.Err()); err != nil {
		return netip.Addr{}, err
	}
	stream := *r.OK()
	defer stream.ResourceDrop()
	p := stream.Subscribe()
	defer p.ResourceDrop()
	for {
		if !time.Now().Before(deadline) {
			return netip.Addr{}, &timeoutError{}
		}
		r := stream.ResolveNextAddress()
		if code := r.Err(); code != nil {
			if *code != wnet.ErrorCodeWouldBlock {
				return netip.Addr{}, socketError(code)
			}
			for !p.Ready() {
				if err := pause(deadline); err != nil {
					return netip.Addr{}, err
				}
			}
			continue
		}
		ip := r.OK().Some()
		if ip == nil {
			return netip.Addr{}, &net.DNSError{Name: name, Err: "no addresses", IsNotFound: true}
		}
		return address(*ip), nil
	}
}

func (wasiBackend) socket(family int) (connection, error) {
	f := wnet.IPAddressFamilyIPv4
	if family == 10 {
		f = wnet.IPAddressFamilyIPv6
	}
	r := tcpcreatesocket.CreateTCPSocket(f)
	if err := socketError(r.Err()); err != nil {
		return nil, err
	}
	return &wasiConnection{socket: *r.OK()}, nil
}

type wasiConnection struct {
	mu                 sync.Mutex
	readMu, writeMu    sync.Mutex
	socket             tcp.TCPSocket
	in                 streams.InputStream
	out                streams.OutputStream
	readable, writable poll.Pollable
	connected, closed  bool
}

func (s *wasiConnection) connect(addr netip.AddrPort, deadline time.Time) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return net.ErrClosed
	}
	if s.connected {
		return errors.New("network: socket already connected")
	}
	if addr.Addr().Zone() != "" {
		return ErrUnsupported
	}
	network := instancenetwork.InstanceNetwork()
	defer network.ResourceDrop()
	r := s.socket.StartConnect(network, socketAddress(addr))
	if err := socketError(r.Err()); err != nil {
		return err
	}
	p := s.socket.Subscribe()
	defer p.ResourceDrop()
	for {
		if !time.Now().Before(deadline) {
			return &timeoutError{}
		}
		r := s.socket.FinishConnect()
		if code := r.Err(); code != nil {
			if *code != wnet.ErrorCodeWouldBlock {
				return socketError(code)
			}
			for !p.Ready() {
				if err := pause(deadline); err != nil {
					return err
				}
			}
			continue
		}
		s.in, s.out = r.OK().F0, r.OK().F1
		s.readable, s.writable = s.in.Subscribe(), s.out.Subscribe()
		s.connected = true
		return nil
	}
}

// ready is checked under the same lock as resource drops. Sleeping never holds
// that lock, so Close can wake a pending read or write without stale handles.
func (s *wasiConnection) wait(read bool, deadline time.Time) error {
	for {
		s.mu.Lock()
		if s.closed {
			s.mu.Unlock()
			return net.ErrClosed
		}
		p := s.writable
		if read {
			p = s.readable
		}
		ready := p.Ready()
		s.mu.Unlock()
		if ready {
			return nil
		}
		if err := pause(deadline); err != nil {
			return err
		}
	}
}

func (s *wasiConnection) read(b []byte, deadline time.Time) (int, error) {
	s.readMu.Lock()
	defer s.readMu.Unlock()
	for {
		s.mu.Lock()
		if s.closed {
			s.mu.Unlock()
			return 0, net.ErrClosed
		}
		if !s.connected {
			s.mu.Unlock()
			return 0, errors.New("network: socket not connected")
		}
		if len(b) == 0 {
			s.mu.Unlock()
			return 0, nil
		}
		if !deadline.IsZero() && !time.Now().Before(deadline) {
			s.mu.Unlock()
			return 0, &timeoutError{}
		}
		r := s.in.Read(uint64(len(b)))
		if e := r.Err(); e != nil {
			err := streamError(*e)
			s.mu.Unlock()
			return 0, err
		}
		n := copy(b, r.OK().Slice())
		s.mu.Unlock()
		if n != 0 {
			return n, nil
		}
		if err := s.wait(true, deadline); err != nil {
			return 0, err
		}
	}
}

func (s *wasiConnection) write(b []byte, deadline time.Time) (int, error) {
	s.writeMu.Lock()
	defer s.writeMu.Unlock()
	if len(b) == 0 {
		s.mu.Lock()
		defer s.mu.Unlock()
		return 0, s.writeState(deadline)
	}
	return writeStream(wasiOutput{s}, b, deadline)
}

// writeState is called with mu held, before touching any host stream resource.
func (s *wasiConnection) writeState(deadline time.Time) error {
	if s.closed {
		return net.ErrClosed
	}
	if !s.connected {
		return errors.New("network: socket not connected")
	}
	if !deadline.IsZero() && !time.Now().Before(deadline) {
		return &timeoutError{}
	}
	return nil
}

type wasiOutput struct{ connection *wasiConnection }

func (out wasiOutput) checkWrite(deadline time.Time) (uint64, error) {
	s := out.connection
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.writeState(deadline); err != nil {
		return 0, err
	}
	r := s.out.CheckWrite()
	if e := r.Err(); e != nil {
		return 0, streamError(*e)
	}
	return *r.OK(), nil
}
func (out wasiOutput) write(b []byte, deadline time.Time) error {
	s := out.connection
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.writeState(deadline); err != nil {
		return err
	}
	r := s.out.Write(cm.ToList(b))
	if e := r.Err(); e != nil {
		return streamError(*e)
	}
	return nil
}
func (out wasiOutput) flush(deadline time.Time) error {
	s := out.connection
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.writeState(deadline); err != nil {
		return err
	}
	r := s.out.Flush()
	if e := r.Err(); e != nil {
		return streamError(*e)
	}
	return nil
}
func (out wasiOutput) wait(deadline time.Time) error {
	return out.connection.wait(false, deadline)
}

func (s *wasiConnection) close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return nil
	}
	s.closed = true
	// Pollables are children of streams. Drop children before their parents.
	if s.connected {
		s.readable.ResourceDrop()
		s.writable.ResourceDrop()
		s.in.ResourceDrop()
		s.out.ResourceDrop()
	}
	s.socket.ResourceDrop()
	return nil
}

func (s *wasiConnection) option(level, option int, value any) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return net.ErrClosed
	}
	if level == 1 && option == 9 {
		v, ok := value.(bool)
		if !ok {
			return ErrUnsupported
		}
		r := s.socket.SetKeepAliveEnabled(v)
		return socketError(r.Err())
	}
	if level == 6 && option == 5 {
		d, err := keepAlivePeriod(value)
		if err != nil {
			return err
		}
		// TinyGo routes SetKeepAlivePeriod through its half-second option.
		// Go defines this period as idle time before the first probe.
		r := s.socket.SetKeepAliveIdleTime(tcp.Duration(d))
		return socketError(r.Err())
	}
	return ErrUnsupported
}

func address(ip wnet.IPAddress) netip.Addr {
	if v := ip.IPv4(); v != nil {
		return netip.AddrFrom4([4]byte(*v))
	}
	v := ip.IPv6()
	var b [16]byte
	for i, word := range *v {
		binary.BigEndian.PutUint16(b[i*2:], word)
	}
	return netip.AddrFrom16(b).Unmap()
}
func socketAddress(addr netip.AddrPort) wnet.IPSocketAddress {
	if addr.Addr().Is4() {
		return wnet.IPSocketAddressIPv4(wnet.IPv4SocketAddress{Port: addr.Port(), Address: wnet.IPv4Address(addr.Addr().As4())})
	}
	b := addr.Addr().As16()
	var words wnet.IPv6Address
	for i := range words {
		words[i] = binary.BigEndian.Uint16(b[i*2:])
	}
	return wnet.IPSocketAddressIPv6(wnet.IPv6SocketAddress{Port: addr.Port(), Address: words})
}
func socketError(code *wnet.ErrorCode) error {
	if code == nil {
		return nil
	}
	if *code == wnet.ErrorCodeAccessDenied {
		return errors.Join(os.ErrPermission, errors.New("network: access-denied"))
	}
	if *code == wnet.ErrorCodeTimeout {
		return &timeoutError{}
	}
	return errors.New("network: " + code.String())
}
func streamError(e streams.StreamError) error {
	if e.Closed() {
		return io.EOF
	}
	h := *e.LastOperationFailed()
	defer h.ResourceDrop()
	// network-error-code is an unstable WASI feature not enabled by this host.
	// Keep stable io/error diagnostics without adding that optional import.
	return errors.New("network: " + h.ToDebugString())
}
