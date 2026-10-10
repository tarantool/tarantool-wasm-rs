package main

import (
	"bytes"
	"compress/flate"
	"compress/gzip"
	"compress/zlib"
	"crypto/tls"
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

	"go.bytecodealliance.org/cm"
)

func probe(name string) string {
	parts := strings.SplitN(name, "|", 2)
	name = parts[0]
	address := "127.0.0.1:18082"
	if len(parts) == 2 {
		address = parts[1]
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
		names, e := net.LookupHost("localhost")
		if e != nil {
			return e.Error()
		}
		return fmt.Sprint(names)
	case "tcp":
		conn, e := net.DialTimeout("tcp", address, 2*time.Second)
		if e != nil {
			return e.Error()
		}
		defer conn.Close()
		conn.SetDeadline(time.Now().Add(2 * time.Second))
		if _, e = conn.Write([]byte("hello")); e != nil {
			return e.Error()
		}
		b := make([]byte, 5)
		_, e = io.ReadFull(conn, b)
		if e != nil {
			return e.Error()
		}
		return string(b)
	case "http":
		c := http.Client{Timeout: 2 * time.Second}
		r, e := c.Get("http://" + address + "/")
		if e != nil {
			return e.Error()
		}
		defer r.Body.Close()
		b, e := io.ReadAll(r.Body)
		if e != nil {
			return e.Error()
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
