local t=require('luatest')
local wasm=require('wasm')
local fio=require('fio')
local socket=require('socket')
local json=require('json')
local g=t.group('go_capabilities')
local observations={external_https={status='skipped',reason='No external service in reproducible tests.'}}
g.after_all(function()
    fio.mktree('guests/go/dist')
    local file=assert(io.open('guests/go/dist/capabilities-observed.json','w'))
    file:write(json.encode(observations));file:close()
end)
local function invoke(name,options)
    local m=wasm.load('guests/go/tests/components/probes/sdk.wasm',options or {})
    local ok,v=pcall(wasm.call,m,'check',name);wasm.drop(m)
    t.assert(ok,tostring(v));return v
end
g.test_compression=function()
    t.assert_equals(invoke('compression'),'ok')
    observations.compression={status='works',scenario='flate, gzip, zlib, crc32, adler32; compile and runtime'}
end
g.test_dns=function()
    local v=invoke('dns')
    -- The selected TinyGo net backend currently returns a missing network device.
    t.assert_str_contains(v,'Netdev not set')
    observations.dns={status='known-limitation',compiled=true,runtime=v}
end
g.test_local_tcp_http_tls=function()
	local echo=assert(socket.tcp_server('127.0.0.1',0,function(client)
		local data=client:read(5,2)
		if data then client:write(data) end
		client:close()
	end))
    local server=assert(socket.tcp_server('127.0.0.1',0,function(client)
        client:read({delimiter='\r\n\r\n',chunk=4096},2)
        client:write('HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\npong')
        client:close()
    end))
    local ok,e=pcall(function()
        local endpoint='127.0.0.1:'..server:name().port
        for _,name in ipairs({'tcp','http','tls'}) do
            local address=name=='tcp' and '127.0.0.1:'..echo:name().port or endpoint
            local v=invoke(name..'|'..address)
            t.assert_str_contains(v,'Netdev not set')
            observations[name]={status='known-limitation',compiled=true,runtime=v,endpoint='local-loopback',
                tls_handshake=name=='tls' and 'not-checked: connection fails before handshake' or nil}
        end
        local v=invoke('tcp|'..endpoint,{inherit_network=false,allow_tcp=false})
        t.assert_str_contains(v,'Netdev not set')
        observations.network_policy={status='not-checked',reason='Toolchain fails before host socket imports.'}
    end)
    server:close();echo:close();t.assert(ok,tostring(e))
end
