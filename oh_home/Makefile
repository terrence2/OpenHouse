all: build/main.js

build/main.js: main.ts
	node_modules/typescript/bin/tsc --sourcemap --module commonjs --outDir build main.ts

vardir:
	mkdir -p /var/run/openhouse/home

check: build/main.js vardir
	test/test.py

clean:
	rm -rfv build
	rm -rfv node_modules
