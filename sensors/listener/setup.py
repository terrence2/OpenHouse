#!/usr/bin/env python2
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from distutils.core import setup
import os
import os.path
import socket

corpus_files = os.listdir('corpus-0')
corpus_files = [os.path.join('corpus-0', fn) for fn in corpus_files]

hostname = socket.gethostname()
conf_file = os.path.join('conf', hostname, 'listener.ini')

setup(name='Listener',
      version='1.0',
      description='MCP Listener',
      author='Terrence Cole',
      py_modules=['listener'],
      scripts=['mcp-listener'],
      data_files=[
          ('/usr/share/mcp/listener/corpus', corpus_files),
          ('/etc/mcp', [conf_file]),
          ('/etc/systemd/system', ['init.d/mcp-listener.service']),
      ]
     )