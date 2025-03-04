#include "tarantool.h"
#include <stdio>

int main() {
	std::cout << box_schema_version() << std::endl;
	return 0;
}
