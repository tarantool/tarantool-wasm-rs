local log = require('log')

box.cfg{}

box.schema.space.create("test_space", {if_not_exists = true})
box.space.test_space:create_index("primary", {parts = {1, "unsigned"}, if_not_exists = true})

tx_test = require('tx_test')
tx_test.run()
tx_test.join()

for _, tuple in box.space.test_space:pairs() do
    log.info("%s", tuple)
end
