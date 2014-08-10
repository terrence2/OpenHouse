DIST=./deploy

all:
	mkdir -p ${DIST}
	rsync -avx bower_components/foundation/ ${DIST}/
	cp -av bower_components/requirejs/require.js ${DIST}/js/
	cp -av bower_components/knockout.js/knockout.js ${DIST}/js/vendor/
	cp -av bower_components/spectrum/spectrum.js ${DIST}/js/vendor/
	cp -av bower_components/spectrum/spectrum.css ${DIST}/css/
	cp -arv src/* ${DIST}/

watch:
	while inotifywait -e close -r src; do make all; done

run:
	(cd ${DIST} && python -m SimpleHTTPServer 8000)
