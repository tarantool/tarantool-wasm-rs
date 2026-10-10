local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local socket = require('socket')
local msgpack = require('msgpack')
local g = t.group('go_sdk')
local root = fio.abspath('guests/go/tests/components')
local function comparable(value)
    if box.error.is(value) then return value:unpack() end
    if type(value) ~= 'table' then return value end
    local result = {}
    for key, item in pairs(value) do result[key] = comparable(item) end
    return result
end
g.before_all(function(cg)
    box.cfg{memtx_use_mvcc_engine=true}
    cg.users = box.schema.space.create('go_users')
    cg.users:create_index('primary')
    cg.users:create_index('name', {parts={{2,'string'}}})
    cg.ids = box.schema.space.create('go_ids'); cg.ids:create_index('primary')
    cg.seq = box.schema.sequence.create('go_seq'); cg.ids:replace{1,cg.seq.id}
    cg.specials = box.schema.space.create('go_specials')
    cg.specials:format({{'id','unsigned'},{'decimal','decimal'},{'uuid','uuid'},
                       {'datetime','datetime'},{'interval','interval'},{'error','any'}})
    cg.specials:create_index('primary')
    cg.bigints = box.schema.space.create('go_bigints'); cg.bigints:create_index('primary')
    wasm.register_callback('go_echo', function(v) return v end)
    wasm.register_callback('go_binary', function(v) assert(require('varbinary').is(v)); return v end)
    wasm.register_callback('go_fail', function() error('Go callback failure') end)
end)
g.before_each(function(cg)
    cg.users:truncate()
    cg.base=wasm.load(fio.pathjoin(root,'base/sdk.wasm'))
    cg.extras=wasm.load(fio.pathjoin(root,'extras/sdk.wasm'))
    wasm.register_callback('go_replace', function(v)
        wasm.register_callback('go_replace', function(x) return x + 2 end)
        return v + 1
    end)
end)
g.after_each(function(cg)
    if cg.base then wasm.drop(cg.base) end
    if cg.extras then wasm.drop(cg.extras) end
end)
g.after_all(function(cg)
    for _,name in ipairs({'go_echo','go_binary','go_fail','go_replace'}) do wasm.unregister_callback(name) end
    cg.users:drop(); cg.ids:drop(); cg.seq:drop(); cg.specials:drop(); cg.bigints:drop()
end)
for _,name in ipairs({'crud','indexes','tuples','keys','sequences','diagnostics','logging','messagepack','extensions','transactionRejected'}) do
    g['test_'..name]=function(cg) t.assert_equals(wasm.call(cg.base,'check',name),'ok') end
end
for _,name in ipairs({'lua-callback','zlib'}) do
    g['test_'..name:gsub('-','_')]=function(cg) t.assert_equals(wasm.call(cg.extras,'check',name),'ok') end
end
g.test_schema_changes=function(cg)
    t.assert_equals(wasm.call(cg.base,'check','crud'),'ok')
    local old=cg.users.id
    cg.users:drop();cg.users=box.schema.space.create('go_users',{id=old+1000})
    cg.users:create_index('primary');cg.users:create_index('name',{id=3,parts={{2,'string'}}})
    t.assert_equals(wasm.call(cg.base,'check','crud'),'ok')
end
g.test_transactions=function(cg)
    local directory=fio.tempdir();local stderr=fio.pathjoin(directory,'stderr')
    local ok,result=pcall(function() return wasm.join(wasm.run(cg.base,{stderr=stderr})) end)
    local file=io.open(stderr,'r');local diagnostic=file and file:read('*a') or ''
    if file then file:close() end;fio.rmtree(directory)
    t.assert(ok,tostring(result)..' '..diagnostic);t.assert(result,diagnostic)
    t.assert_equals(cg.users:get(11),{11,'commit'});t.assert_equals(cg.users:get(12),nil)
    t.assert_equals(cg.users:get(13),nil);t.assert_equals(cg.users:get(14),{14,'explicit'})
end
g.test_direct_build=function(cg)
    local m=wasm.load(fio.pathjoin(root,'base/direct.wasm'))
    local ok,v=pcall(wasm.call,m,'check','crud');wasm.drop(m)
    t.assert(ok,tostring(v));t.assert_equals(v,'ok')
end
g.test_sessions=function(cg)
    local directory=fio.tempdir();local path=fio.pathjoin(directory,'go.sock')
    box.cfg{listen=path};local id
    local on_connect=function() id=box.session.id() end;box.session.on_connect(on_connect)
    local client=assert(socket.tcp_connect('unix/',path));t.assert_equals(#client:read(128),128)
    box.session.on_connect(nil,on_connect);cg.ids:replace{2,id}
    local received;local watcher=box.watch('go_event',function(_,v) received=v end)
    local ok,v=pcall(wasm.call,cg.base,'check','sessions')
    if ok then
        for sync=77,78 do
            local length=msgpack.decode(assert(client:read(5)));local packet=assert(client:read(length))
            local header,offset=msgpack.decode(packet);t.assert_equals(header,{[0]=0,[1]=sync})
            if sync==77 then t.assert_equals(msgpack.decode(packet,offset),{[0x30]='sdk'})
            else t.assert_equals(offset,#packet+1) end
        end
        t.helpers.retrying({timeout=2},function() t.assert_equals(received,{answer=42}) end)
    end
    watcher:unregister();client:close();box.cfg{listen=''};fio.rmtree(directory)
    t.assert(ok,tostring(v));t.assert_equals(v,'ok')
end
g.test_cross_language_exchange=function(cg)
    t.assert_equals(wasm.call(cg.base,'check','extensions'),'ok')
    for _,language in ipairs({'python','js'}) do
        local m=wasm.load(fio.pathjoin(root,language..'-peer/sdk.wasm'))
        local ok,v=pcall(wasm.call,m,'exchange')
        if ok then
            for _,row in ipairs({cg.specials:get(1):totable(),cg.bigints:get(1):totable(),
                               {box.NULL,require('varbinary').new('\0\255'),{}, {a=1}}}) do
                local encoded=msgpack.encode(row)
                local bytes={encoded:byte(1,-1)}
                local out=wasm.call(m,'roundtrip',bytes)
                local gobytes=wasm.call(cg.base,'roundtrip',out)
                local decoded=msgpack.decode(string.char(unpack(gobytes)))
                t.assert_equals(comparable(decoded),comparable(row))
            end
        end
        wasm.drop(m);t.assert(ok,tostring(v));t.assert_equals(v,'ok')
    end
    -- Both peers write values that Go then decodes, including extensions.
    for _,id in ipairs({2,3}) do
        local row=cg.specials:get(id):totable();local encoded=msgpack.encode(row)
        local bytes=wasm.call(cg.base,'roundtrip',{encoded:byte(1,-1)})
        t.assert_equals(comparable(msgpack.decode(string.char(unpack(bytes)))),comparable(row))
    end
end
