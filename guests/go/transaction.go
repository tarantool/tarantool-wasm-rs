package twr

import (
	"errors"
	"fmt"
)

// TransactionOptions changes only the current transaction, never runtime config.
type TransactionOptions struct {
	Isolation   *TransactionIsolation
	Synchronous bool
}

func (c *Client) TransactionID() (uint64, bool, error) {
	v, e := c.host.Call("transaction.ID")
	if e != nil {
		return 0, false, e
	}
	if v == nil {
		return 0, false, nil
	}
	id, ok := v.(uint64)
	if !ok {
		return 0, false, fmt.Errorf("twr: invalid transaction ID binding")
	}
	return id, true, nil
}
func (c *Client) TransactionIsolation() (TransactionIsolation, bool, error) {
	v, e := c.host.Call("transaction.Isolation")
	if e != nil {
		return 0, false, e
	}
	if v == nil {
		return 0, false, nil
	}
	level, ok := v.(TransactionIsolation)
	if !ok {
		return 0, false, fmt.Errorf("twr: invalid transaction isolation binding")
	}
	return level, true, nil
}
func (c *Client) Begin(o TransactionOptions) error {
	if c.managed {
		return ErrNestedTransaction
	}
	return c.begin(o)
}
func (c *Client) begin(o TransactionOptions) error {
	_, active, e := c.TransactionID()
	if e != nil {
		return e
	}
	if active {
		return ErrNestedTransaction
	}
	if e = c.do("transaction.Begin"); e != nil {
		return e
	}
	if o.Isolation != nil {
		e = c.SetIsolation(*o.Isolation)
	}
	if e == nil && o.Synchronous {
		e = c.MakeSynchronous()
	}
	if e != nil {
		return errors.Join(e, c.do("transaction.Rollback"))
	}
	return nil
}
func (c *Client) Commit() error {
	if c.managed {
		return ErrManagedTransaction
	}
	return c.do("transaction.Commit")
}
func (c *Client) Rollback() error {
	if c.managed {
		return ErrManagedTransaction
	}
	return c.do("transaction.Rollback")
}
func (c *Client) SetIsolation(level TransactionIsolation) error {
	if level > TransactionIsolationLinearizable {
		return fmt.Errorf("twr: invalid transaction isolation %d", level)
	}
	return c.do("transaction.SetIsolation", level)
}
func (c *Client) MakeSynchronous() error { return c.do("transaction.MakeSynchronous") }

// Transaction executes fn synchronously on the current host fiber. Do not use
// goroutines, yield, or finish/replace the transaction through raw APIs inside fn.
// IDs detect raw completion/replacement; WIT has no Go goroutine identity API.
func (c *Client) Transaction(fn func() error, o TransactionOptions) (err error) {
	if fn == nil {
		return fmt.Errorf("twr: nil transaction callback")
	}
	if c.managed {
		return ErrNestedTransaction
	}
	if err = c.begin(o); err != nil {
		return err
	}
	id, active, e := c.TransactionID()
	if e != nil || !active {
		return errors.Join(e, ErrTransactionChanged, c.do("transaction.Rollback"))
	}
	c.managed = true
	returned := false
	defer func() {
		p := recover()
		c.managed = false
		if returned && p == nil && err == nil {
			return
		}
		var cleanup error
		func() {
			finished := false
			defer func() {
				if q := recover(); q != nil || !finished {
					cleanup = fmt.Errorf("twr: rollback panic: %v", q)
				}
			}()
			now, present, e := c.TransactionID()
			if e != nil {
				cleanup = e
				finished = true
				return
			}
			if !present || now != id {
				if err == nil {
					err = ErrTransactionChanged
				}
				finished = true
				return
			}
			cleanup = c.do("transaction.Rollback")
			finished = true
		}()
		if p != nil || !returned {
			panic(p)
		}
		err = errors.Join(err, cleanup)
	}()
	err = func() error {
		if e := fn(); e != nil {
			return e
		}
		now, present, e := c.TransactionID()
		if e != nil {
			return e
		}
		if !present || now != id {
			return ErrTransactionChanged
		}
		return c.do("transaction.Commit")
	}()
	returned = true
	return err
}
