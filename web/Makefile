DIST=./dist

all:
	mkdir -p dist
	rsync -avx vendor/foundation/ dist/
	cp -av vendor/knockout-*.js dist/js/vendor/
	cp -arv src/* dist/

watch:
	while inotifywait -e close -r src; do make all; done

run:
	(cd dist && python -m SimpleHTTPServer 8000)
