//go:build tinygo.wasm && wasip2

package network

import (
	"errors"
	"net/netip"
	"sync"
	"time"
	_ "unsafe" // TinyGo's documented netdev registration hook uses go:linkname.
)

// netdever mirrors TinyGo 0.43's private interface. Keeping the hook here avoids
// importing hardware drivers into this WASI-only module.
type netdever interface {
	GetHostByName(string) (netip.Addr, error)
	Addr() (netip.Addr, error)
	Socket(int, int, int) (int, error)
	Bind(int, netip.AddrPort) error
	Connect(int, string, netip.AddrPort) error
	Listen(int, int) error
	Accept(int) (int, netip.AddrPort, error)
	Send(int, []byte, int, time.Time) (int, error)
	Recv(int, []byte, int, time.Time) (int, error)
	Close(int) error
	SetSockOpt(int, int, int, any) error
}

//go:linkname useNetdev net.useNetdev
func useNetdev(netdever)

var installation struct {
	sync.Mutex
	driver *device
}

// Enable installs the WASI driver for this component. Repeating the same
// configuration is safe; replacing a live driver is rejected. No host calls
// occur until a network operation. Build with TinyGo's asyncify scheduler when
// using net/http; this mode does not provide the database SDK's panic guarantees.
func Enable(options Options) error {
	d, err := newDevice(wasiBackend{}, options)
	if err != nil {
		return err
	}
	installation.Lock()
	defer installation.Unlock()
	if installation.driver != nil {
		if installation.driver.timeout == d.timeout {
			return nil
		}
		return errors.New("network: driver already enabled with different options")
	}
	useNetdev(d)
	installation.driver = d
	return nil
}
