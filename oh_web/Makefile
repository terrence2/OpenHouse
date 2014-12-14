build: build/oh_web.js

build/oh_web.js: Makefile src/*.js
	mkdir -p build
	#./node_modules/.bin/browserify src/main.ts -r jquery -p tsify --debug -o build/oh_web.js
	./node_modules/.bin/browserify src/main.js -r jquery -r jss -r ramda --debug -o build/oh_web.js

watch:
	while inotifywait -e close -r src; do make; done
