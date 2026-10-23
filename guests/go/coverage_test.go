package twr_test

import (
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"regexp"
	"strconv"
	"strings"
	"testing"

	twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
	cb "github.com/tarantool/tarantool-wasm-rs/guests/go/luacallback"
	rawcb "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/docs/adder/lua-callback"
	zlib "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/docs/adder/zlib"
	boxtuple "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/box-tuple"
	database "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/database"
	rawerror "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/error"
	index "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/index"
	keydef "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/key-def"
	log "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/log"
	metrics "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/metrics"
	msgpack "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/msgpack"
	sequence "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/sequence"
	session "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/session"
	transaction "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/transaction"
	tupleformat "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/tuple-format"
)

// Every entry contains real method/function references, checked by the Go compiler.
// Raw-only zlib intentionally has no public convenience method.
var apiCoverage = map[string]struct{ SDK, Raw any }{
	"metrics/uptime":               {(*twr.Client).Uptime, metrics.Uptime},
	"box-tuple/iterator.new":       {(*twr.Tuple).Cursor, boxtuple.IteratorNew},
	"box-tuple/iterator.position":  {(*twr.TupleCursor).Position, (boxtuple.Iterator).Position},
	"box-tuple/iterator.rewind":    {(*twr.TupleCursor).Rewind, (boxtuple.Iterator).Rewind},
	"box-tuple/iterator.seek":      {(*twr.TupleCursor).Seek, (boxtuple.Iterator).Seek},
	"box-tuple/iterator.next":      {(*twr.TupleCursor).Next, (boxtuple.Iterator).Next},
	"box-tuple/iterator.drop":      {(*twr.TupleCursor).Close, (boxtuple.Iterator).ResourceDrop},
	"box-tuple/new":                {(*twr.Client).NewTuple, boxtuple.New},
	"box-tuple/retain":             {(*twr.Tuple).Copy, boxtuple.Retain},
	"box-tuple/release":            {(*twr.Tuple).Close, boxtuple.Release},
	"box-tuple/field-count":        {(*twr.Tuple).FieldCount, boxtuple.FieldCount},
	"box-tuple/byte-size":          {(*twr.Tuple).ByteSize, boxtuple.ByteSize},
	"box-tuple/to-bytes":           {(*twr.Tuple).Bytes, boxtuple.ToBytes},
	"box-tuple/format":             {(*twr.Tuple).Format, boxtuple.Format},
	"box-tuple/field":              {(*twr.Tuple).Field, boxtuple.Field},
	"box-tuple/field-by-path":      {(*twr.Tuple).FieldByPath, boxtuple.FieldByPath},
	"box-tuple/update":             {(*twr.Tuple).Update, boxtuple.Update},
	"box-tuple/upsert":             {(*twr.Tuple).Upsert, boxtuple.Upsert},
	"box-tuple/validate":           {(*twr.Tuple).Validate, boxtuple.Validate},
	"box-tuple/compare":            {(*twr.Tuple).Compare, boxtuple.Compare},
	"box-tuple/compare-with-key":   {(*twr.Tuple).CompareWithKey, boxtuple.CompareWithKey},
	"database/schema-version":      {(*twr.Client).SchemaVersion, database.SchemaVersion},
	"database/space-by-name":       {(*twr.Space).ID, database.SpaceByName},
	"database/index-by-name":       {(*twr.Index).ID, database.IndexByName},
	"database/insert":              {(*twr.Space).Insert, database.Insert},
	"database/update":              {(*twr.Index).Update, database.Update},
	"database/replace":             {(*twr.Space).Replace, database.Replace},
	"database/upsert":              {(*twr.Index).Upsert, database.Upsert},
	"database/delete":              {(*twr.Index).Delete, database.Delete},
	"database/truncate":            {(*twr.Space).Truncate, database.Truncate},
	"error/new":                    {(*twr.Client).NewError, rawerror.New},
	"error/new-with-location":      {(*twr.Client).NewErrorWithLocation, rawerror.NewWithLocation},
	"error/set":                    {(*twr.Client).SetError, rawerror.Set},
	"error/last":                   {(*twr.Client).LastError, rawerror.Last},
	"error/clear":                  {(*twr.Client).ClearError, rawerror.Clear},
	"error/to-string":              {(*twr.Client).FormatError, rawerror.ToString},
	"index/iterator.new":           {(*twr.Index).Iter, index.IteratorNew},
	"index/iterator.next":          {(*twr.IndexIterator).Next, (index.Iterator).Next},
	"index/iterator.drop":          {(*twr.IndexIterator).Close, (index.Iterator).ResourceDrop},
	"index/tuple-count":            {(*twr.Index).TupleCount, index.TupleCount},
	"index/memory-size":            {(*twr.Index).MemorySize, index.MemorySize},
	"index/random":                 {(*twr.Index).Random, index.Random},
	"index/get":                    {(*twr.Index).Get, index.Get},
	"index/min":                    {(*twr.Index).Min, index.Min},
	"index/max":                    {(*twr.Index).Max, index.Max},
	"index/count":                  {(*twr.Index).Count, index.Count},
	"key-def/new":                  {(*twr.Client).NewKeyDef, keydef.New},
	"key-def/duplicate":            {(*twr.KeyDef).Copy, keydef.Duplicate},
	"key-def/merge":                {(*twr.KeyDef).Merge, keydef.Merge},
	"key-def/release":              {(*twr.KeyDef).Close, keydef.Release},
	"key-def/dump-parts":           {(*twr.KeyDef).Parts, keydef.DumpParts},
	"key-def/validate-key":         {(*twr.KeyDef).ValidateKey, keydef.ValidateKey},
	"key-def/validate-full-key":    {(*twr.KeyDef).ValidateKey, keydef.ValidateFullKey},
	"key-def/part-count":           {(*twr.KeyDef).PartCount, keydef.PartCount},
	"key-def/extract-key":          {(*twr.KeyDef).ExtractKey, keydef.ExtractKey},
	"key-def/validate-tuple":       {(*twr.KeyDef).ValidateTuple, keydef.ValidateTuple},
	"log/write":                    {(*twr.Client).Log, log.Write},
	"msgpack/from-json":            {(*twr.Client).FromJSON, msgpack.FromJSON},
	"msgpack/to-json":              {(*twr.Client).ToJSON, msgpack.ToJSON},
	"sequence/next":                {(*twr.Sequence).Next, sequence.Next},
	"sequence/current":             {(*twr.Sequence).Current, sequence.Current},
	"sequence/set":                 {(*twr.Sequence).Set, sequence.Set},
	"sequence/reset":               {(*twr.Sequence).Reset, sequence.Reset},
	"session/current":              {(*twr.Client).CurrentSession, session.Current},
	"session/broadcast":            {(*twr.Client).BroadcastLua, session.Broadcast},
	"session/iproto-send":          {(*twr.Session).Send, session.IprotoSend},
	"transaction/id":               {(*twr.Client).TransactionID, transaction.ID},
	"transaction/isolation":        {(*twr.Client).TransactionIsolation, transaction.Isolation},
	"transaction/begin":            {(*twr.Client).Begin, transaction.Begin},
	"transaction/commit":           {(*twr.Client).Commit, transaction.Commit},
	"transaction/rollback":         {(*twr.Client).Rollback, transaction.Rollback},
	"transaction/set-isolation":    {(*twr.Client).SetIsolation, transaction.SetIsolation},
	"transaction/make-synchronous": {(*twr.Client).MakeSynchronous, transaction.MakeSynchronous},
	"tuple-format/default":         {(*twr.Client).DefaultTupleFormat, tupleformat.Default},
	"tuple-format/new":             {(*twr.Client).NewTupleFormat, tupleformat.New},
	"tuple-format/retain":          {(*twr.TupleFormat).Copy, tupleformat.Retain},
	"tuple-format/release":         {(*twr.TupleFormat).Close, tupleformat.Release},
	"lua-callback/open":            {cb.Open, rawcb.Open},
	"lua-callback/handler.call":    {(*cb.Callback).Call, (rawcb.Handler).Call},
	"lua-callback/handler.drop":    {(*cb.Callback).Close, (rawcb.Handler).ResourceDrop},
	"zlib/compress":                {nil, zlib.Compress},
	"zlib/decompress":              {nil, zlib.Decompress},
	"zlib/crc32":                   {nil, zlib.Crc32},
	"zlib/adler32":                 {nil, zlib.Adler32},
}

func witFunctions(t *testing.T, source string) map[string]bool {
	t.Helper()
	source = regexp.MustCompile(`(?m)///[^
]*`).ReplaceAllString(source, "")
	iface, resource := "", ""
	out := map[string]bool{}
	for _, line := range strings.Split(source, "\n") {
		line = strings.TrimSpace(line)
		if m := regexp.MustCompile(`^interface ([a-z-]+)`).FindStringSubmatch(line); m != nil {
			iface = m[1]
			resource = ""
			continue
		}
		if m := regexp.MustCompile(`^resource ([a-z-]+)`).FindStringSubmatch(line); m != nil {
			resource = m[1]
			out[iface+"/"+resource+".drop"] = true
			continue
		}
		if line == "}" {
			if resource != "" {
				resource = ""
			} else {
				iface = ""
			}
			continue
		}
		if iface != "" {
			if m := regexp.MustCompile(`^([a-z0-9-]+): (?:static )?func`).FindStringSubmatch(line); m != nil {
				fn := m[1]
				if resource != "" {
					fn = resource + "." + fn
				}
				out[iface+"/"+fn] = true
			}
		}
	}
	return out
}

func TestReleasedWITCoverage(t *testing.T) {
	// SDK builds render the locked registry dependency for this coverage check.
	path := os.Getenv("TWR_GO_WIT")
	if path == "" {
		path = "dist/released-tarantool.wit"
	}
	b, e := os.ReadFile(path)
	if e != nil {
		t.Fatal(e)
	}
	host, e := os.ReadFile("wit/runtime/host.wit")
	if e != nil {
		t.Fatal(e)
	}
	functions := witFunctions(t, string(b)+"\n"+string(host))
	for fn := range functions {
		entry, ok := apiCoverage[fn]
		if !ok || entry.Raw == nil {
			t.Errorf("WIT function %s lacks SDK/raw mapping", fn)
		}
		if !strings.HasPrefix(fn, "zlib/") && entry.SDK == nil {
			t.Errorf("%s lacks public SDK method", fn)
		}
	}
	for fn := range apiCoverage {
		if !functions[fn] {
			t.Errorf("coverage maps function absent from current WIT: %s", fn)
		}
	}
	t.Logf("covered %d WIT functions and resource drops", len(functions))
}

// Check the adapter too: compile-checked public/raw references alone would not
// detect a missing dispatcher branch between those two layers.
func TestBindingCoverage(t *testing.T) {
	dispatch := map[string]bool{}
	file, err := parser.ParseFile(token.NewFileSet(), "bindings/host.go", nil, 0)
	if err != nil {
		t.Fatal(err)
	}
	ast.Inspect(file, func(node ast.Node) bool {
		if clause, ok := node.(*ast.CaseClause); ok {
			for _, expression := range clause.List {
				literal, ok := expression.(*ast.BasicLit)
				if !ok || literal.Kind != token.STRING {
					continue
				}
				name, err := strconv.Unquote(literal.Value)
				if err != nil {
					t.Fatal(err)
				}
				dispatch[name] = true
			}
		}
		return true
	})
	pascal := func(name string) string {
		if name == "id" {
			return "ID"
		}
		parts := strings.Split(name, "-")
		for i, part := range parts {
			if part == "json" {
				parts[i] = "JSON"
			} else {
				parts[i] = strings.ToUpper(part[:1]) + part[1:]
			}
		}
		return strings.Join(parts, "")
	}
	for name := range apiCoverage {
		parts := strings.SplitN(name, "/", 2)
		if parts[0] == "zlib" || parts[0] == "lua-callback" {
			continue // Optional adapters have their own runtime and unit tests.
		}
		operation := pascal(parts[1])
		if method := strings.SplitN(parts[1], ".", 2); len(method) == 2 {
			switch method[1] {
			case "new":
				operation = pascal(method[0]) + "New"
			case "drop":
				operation = method[0] + ".ResourceDrop"
			default:
				operation = method[0] + "." + pascal(method[1])
			}
		}
		operation = parts[0] + "." + operation
		if !dispatch[operation] {
			t.Errorf("%s lacks binding dispatch %s", name, operation)
		}
		delete(dispatch, operation)
	}
	for operation := range dispatch {
		t.Errorf("binding dispatch %s has no WIT mapping", operation)
	}
}
