build: build/oh.js build/oh_mobile.js

build/oh.js: Makefile src/*.js
	mkdir -p build
	./node_modules/.bin/browserify src/main.js -r jquery -r jss -r ramda --debug -o build/oh.js

build/oh_mobile.js: Makefile src/*.js
	mkdir -p build
	./node_modules/.bin/browserify src/mobile.js --debug -o build/oh_mobile.js

watch:
	while inotifywait -e close -r src; do make; done
