#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import json
import os
import os.path
import stat

from pprint import pprint

from flask import Flask, abort, redirect, request
from flask.ext import restful

app = Flask("mcp-api")
api = restful.Api(app)


def get_entries_at_path(path: str) -> {}:
    out = {}
    entries = os.listdir(path)
    for entry in entries:
        out[entry] = {}
        child_path = os.path.join(path, entry)
        statbuf = os.stat(child_path)
        out[entry]['readable'] = bool(statbuf.st_mode & stat.S_IRUSR)
        out[entry]['writable'] = bool(statbuf.st_mode & stat.S_IWUSR)
        if stat.S_ISDIR(statbuf.st_mode):
            out[entry]['file_type'] = 'directory'
            out[entry]['subdirs'] = get_entries_at_path(child_path)
        else:
            assert stat.S_ISREG(statbuf.st_mode)
            out[entry]['file_type'] = 'file'
    return out


@app.route('/structure')
def get_structure():
    data = json.dumps(get_entries_at_path('/things'))
    pprint(len(data))
    if 'callback' in request.args:
        data = '{}(\n{}\n);'.format(request.args['callback'], data)
    return data


@app.route('/writefiles')
def do_writefiles():
    data = json.loads(request.args['data'])
    for filename, value in data.items():
        pathname = os.path.realpath(os.path.join('/things', filename))
        if not pathname.startswith('/things'):
            abort(404)
            return
    for filename, value in data.items():
        pathname = os.path.realpath(os.path.join('/things', filename))
        with open(pathname, 'w') as fp:
            fp.write(value)
    if 'callback' in request.args:
        return '{}();'.format(request.args['callback'])
    return ''


@app.route('/readfiles')
def do_readfiles():
    data = json.loads(request.args['data'])
    for filename in data:
        pathname = os.path.realpath(os.path.join('/things', filename))
        if not pathname.startswith('/things'):
            abort(404)
            return
    out = {}
    for filename in data:
        pathname = os.path.realpath(os.path.join('/things', filename))
        with open(pathname, 'r') as fp:
            raw = fp.read()
            # Note: our fs binding may lie about the size for effectful or slow queries.
            processed = raw.strip('\u0000').strip()
            out[filename] = processed
    pprint(out)
    result = json.dumps({'data': out})
    if 'callback' in request.args:
        result = '{}(\n{}\n);'.format(request.args['callback'], result)
    return result


if __name__ == "__main__":
    app.run(debug=True)