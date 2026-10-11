package main

import (
	"bytes"
	"compress/flate"
	"compress/gzip"
	"compress/zlib"
	"crypto/tls"
	"errors"
	world "example/go-probes/internal/example/go-probes/v0.1.0/sdk-test"
	"example/go-probes/internal/wasi/cli/v0.2.12/run"
	"fmt"
	"hash/adler32"
	"hash/crc32"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/network"
	"go.bytecodealliance.org/cm"
)

func probe(name string) string {
	parts := strings.SplitN(name, "|", 2)
	name = parts[0]
	address := "127.0.0.1:18082"
	if len(parts) == 2 {
		address = parts[1]
	}
	if name == "unconfigured" {
		_, err := net.LookupHost("localhost")
		if err != nil {
			return err.Error()
		}
		return "unexpected network driver"
	}
	if name != "compression" {
		if err := network.Enable(network.Options{Timeout: 2 * time.Second}); err != nil {
			return err.Error()
		}
	}
	switch name {
	case "compression":
		data := []byte("standard Go compression")
		for _, kind := range []string{"gzip", "zlib", "flate"} {
			var b bytes.Buffer
			var w io.WriteCloser
			var e error
			switch kind {
			case "gzip":
				w = gzip.NewWriter(&b)
			case "zlib":
				w = zlib.NewWriter(&b)
			case "flate":
				w, e = flate.NewWriter(&b, 6)
			}
			if e != nil {
				return e.Error()
			}
			if _, e = w.Write(data); e != nil {
				return e.Error()
			}
			if e = w.Close(); e != nil {
				return e.Error()
			}
			var r io.ReadCloser
			switch kind {
			case "gzip":
				r, e = gzip.NewReader(&b)
			case "zlib":
				r, e = zlib.NewReader(&b)
			case "flate":
				r = flate.NewReader(&b)
			}
			if e != nil {
				return e.Error()
			}
			out, e := io.ReadAll(r)
			r.Close()
			if e != nil || !bytes.Equal(out, data) {
				return "compression mismatch"
			}
		}
		vector := []byte("123456789")
		if crc32.ChecksumIEEE(vector) != 0xcbf43926 || adler32.Checksum(vector) != 0x091e01de {
			return "checksum mismatch"
		}
		return "ok"
	case "dns":
		host := "localhost"
		if len(parts) == 2 {
			host = address
		}
		names, e := net.LookupHost(host)
		if e != nil {
			return e.Error()
		}
		return fmt.Sprint(names)
	case "tcp", "tcp-timeout", "tcp-close", "tcp-eof", "tcp-options", "tcp-write-reset":
		conn, e := net.DialTimeout("tcp", address, 2*time.Second)
		if e != nil {
			return e.Error()
		}
		defer conn.Close()
		conn.SetDeadline(time.Now().Add(2 * time.Second))
		if name == "tcp-options" {
			tcp := conn.(*net.TCPConn)
			for _, enable := range []bool{true, false} {
				if err := tcp.SetKeepAlive(enable); err != nil {
					return err.Error()
				}
			}
			if err := tcp.SetKeepAlivePeriod(1500 * time.Millisecond); err != nil {
				return err.Error()
			}
			return "ok"
		}
		if name == "tcp-write-reset" {
			payload := bytes.Repeat([]byte{'x'}, 8*1024*1024)
			n, err := conn.Write(payload)
			if err != nil && n >= 0 && n <= len(payload) {
				return "write-error"
			}
			return "unexpected successful write"
		}
		if name == "tcp-timeout" {
			started := time.Now()
			conn.SetReadDeadline(time.Now().Add(50 * time.Millisecond))
			_, err := conn.Read(make([]byte, 1))
			// TinyGo's OpError has Timeout but lacks the deprecated Temporary method.
			var timeout interface{ Timeout() bool }
			if errors.As(err, &timeout) && timeout.Timeout() {
				if time.Since(started) >= time.Second {
					return "read deadline did not bound the operation"
				}
				return "timeout"
			}
			return fmt.Sprint(err)
		}
		if name == "tcp-close" {
			started := time.Now()
			go func() { time.Sleep(25 * time.Millisecond); conn.Close() }()
			_, err := conn.Read(make([]byte, 1))
			if errors.Is(err, net.ErrClosed) {
				if time.Since(started) >= time.Second {
					return "close did not wake the reader"
				}
				return "closed"
			}
			return fmt.Sprint(err)
		}
		if name == "tcp-eof" {
			b, err := io.ReadAll(conn)
			if err != nil {
				return err.Error()
			}
			return string(b)
		}
		if _, e = conn.Write([]byte("hello")); e != nil {
			return e.Error()
		}
		b := make([]byte, 5)
		_, e = io.ReadFull(conn, b)
		if e != nil {
			return e.Error()
		}
		return string(b)
	case "tcp-reuse":
		for i := 0; i < 20; i++ {
			if value := probe("tcp-eof|" + address); value != "done" {
				return value
			}
		}
		return "done"
	case "http", "http-timeout":
		c := http.Client{}
		if name == "http-timeout" {
			c.Timeout = 50 * time.Millisecond
		}
		started := time.Now()
		r, e := c.Get("http://" + address + "/")
		if e != nil {
			return e.Error()
		}
		defer r.Body.Close()
		b, e := io.ReadAll(r.Body)
		if e != nil {
			return e.Error()
		}
		if name == "http-timeout" {
			if string(b) == "pong" && time.Since(started) >= 100*time.Millisecond {
				return "timeout-ignored"
			}
			return "unexpected HTTP timeout behavior"
		}
		return string(b)
	case "tls":
		dialer := net.Dialer{Timeout: 2 * time.Second}
		c, e := tls.DialWithDialer(&dialer, "tcp", address, &tls.Config{MinVersion: tls.VersionTLS12})
		if e != nil {
			return e.Error()
		}
		c.Close()
		return "ok"
	}
	return "unknown probe"
}
func init() {
	world.Exports.Check = probe
	world.Exports.Roundtrip = func(b cm.List[uint8]) cm.List[uint8] { return b }
	run.Exports.Run = func() cm.BoolResult { fmt.Println(probe("compression")); return 0 }
}
func main() {}
