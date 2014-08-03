# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
try:
    from enum import Enum
except ImportError:
    class Enum:
        pass

import errno
import faulthandler
import logging
import stat

from datetime import datetime

from pprint import pprint

import llfuse

faulthandler.enable()
log = logging.getLogger('fs')


class NodeType(Enum):
    Unknown = 0
    Regular = stat.S_IFREG
    Directory = stat.S_IFDIR


# Bump allocate unique inodes.
LAST_INODE = 1


class Node:
    def __init__(self):
        super().__init__()

        # Automatically assign unique inodes.
        global LAST_INODE
        self.inode = LAST_INODE
        LAST_INODE += 1

        # Raw file attributes. Some of these must be overridden by subclasses.
        # We represent stat bitfields like st_mode split out and use properties to combine them for display.
        self.permission_ = 0o600
        self.mode_ = NodeType.Unknown
        self.nlink = 2
        self.ctime = datetime.now()
        self.mtime = datetime.now()
        self.atime = datetime.now()

    def update_atime(self):
        """Update the atime."""
        self.atime_ = datetime.now()

    @property
    def mode(self):
        assert self.mode_ is not None
        return self.permission_ | self.mode_

    @property
    def type(self) -> NodeType:
        return self.mode_


class File(Node):
    def __init__(self, read_function: callable, write_function: callable, fixed_size: int=-1):
        """
        A virtual file. Provide a read function and a write function to do whatever is needed.

        fixed_size:int -- Normally the stat struct's size is filled with the size of the string
                          returned by a call to |read_function|. If calling the |read_function|
                          has side-effects or is slow for some other reason, it is best to
                          provide |fixed_size|, which will be used instead. Note: having an
                          incorrect size will break some tools, but generally not badly enough
                          to make them unusable.
        """
        super().__init__()

        # Required overrides.
        self.mode_ = stat.S_IFREG

        # properties
        self.read_function_ = read_function
        self.write_function_ = write_function

        self.permission_ = 0
        if self.read_function_:
            self.permission_ |= 0o400
        if self.write_function_:
            self.permission_ |= 0o200

        self.fixed_size_ = fixed_size

    def guess_size(self) -> int:
        if self.fixed_size_ != -1:
            return self.fixed_size_
        if not self.read_function_:
            return 0
        return len(self.read_function_().encode('UTF-8'))

    def read(self) -> str:
        self.update_atime()
        if self.read_function_:
            return self.read_function_()
        return ""

    def write(self, data: str):
        self.update_atime()
        if self.write_function_:
            try:
                return self.write_function_(data)
            except Exception as e:
                log.exception(e)
                return errno.EFAULT
        return errno.EPERM


class StaticFile(File):
    def __init__(self, static_data: str):
        self.static_data_ = static_data
        super().__init__(self._read, None)

    def _read(self) -> str:
        return self.static_data_


class Directory(Node):
    def __init__(self):
        super().__init__()

        # Required overrides.
        self.mode_ = stat.S_IFDIR

        # The content of this directory.
        self.entries_ = {  # {str: Node}
            '.': self,
        }

    def add_file(self, name: str, node: File) -> File:
        self.update_atime()
        self.entries_[name] = node
        return node

    def add_subdir(self, name: str, directory):
        self.update_atime()
        self.entries_[name] = directory
        self.nlink += 1
        directory.entries_['..'] = self
        return directory

    def lookup(self, name: str) -> Node:
        self.update_atime()
        return self.entries_[name]

    def listdir(self) -> [str]:
        self.update_atime()
        return list(self.entries_.keys())

    def guess_size(self) -> int:
        return sum((len(k) + 1 for k in self.entries_.keys()))

    def read(self):
        return errno.EINVAL
    def write(self, data):
        return errno.EINVAL


class FileSystem(llfuse.Operations):
    """
    A generic in-memory file-system that gets its layout from the graph of File and Directory nodes that have been
    added to it and its data from making calls on those nodes.
    """
    def __init__(self, mount_path: str='/mnt'):
        super().__init__()

        self.ready_ = False  # Signals initialization of the fuse layer.
        self.mount_path = mount_path

        # Create the root.
        self.root_ = Directory()
        assert self.root_.inode == 1

        # Map the inodes the system gives us to the relevant nodes.
        self.inode_to_node_ = {1: self.root_}  # ino:int => object

    def root(self) -> Directory:
        return self.root_

    def _getattr(self, node: Node):
        self.inode_to_node_[node.inode] = node
        entry = llfuse.EntryAttributes()
        entry.st_ino = node.inode
        entry.generation = 0
        entry.entry_timeout = 300
        entry.attr_timeout = 300
        entry.st_uid = 1000
        entry.st_gid = 1000
        entry.st_rdev = 0
        entry.st_blksize = 4096
        entry.st_size = node.guess_size()
        entry.st_blocks = entry.st_size // entry.st_blksize + (1 if entry.st_size % entry.st_blksize > 0 else 0)
        entry.st_atime = int(node.atime.timestamp())
        entry.st_mtime = int(node.mtime.timestamp())
        entry.st_ctime = int(node.ctime.timestamp())
        entry.st_mode = node.mode
        entry.st_nlink = node.nlink
        return entry

    def getattr(self, inode):
        log.debug("FS:getattr: {}".format(inode))
        node = self.inode_to_node_[inode]
        return self._getattr(node)

    def opendir(self, inode):
        assert inode in self.inode_to_node_
        return inode

    def readdir(self, inode, off):
        node = self.inode_to_node_[inode]
        entries = node.listdir()
        for i, name in enumerate(entries[off:], off):
            child = node.lookup(name)
            stat = self._getattr(child)
            yield (name.encode('UTF-8'), stat, i + 1)

    def open(self, inode, flags):
        assert inode in self.inode_to_node_
        return inode

    def read(self, fh, offset, length):
        node = self.inode_to_node_[fh]
        data = node.read()
        if isinstance(data, int):
            raise llfuse.FUSEError(data)
        if isinstance(data, str):
            data = data.encode('UTF-8')
        assert isinstance(data, bytes)
        return data[offset:offset + length]

    def write(self, fh, offset, buf):
        node = self.inode_to_node_[fh]
        data = buf.decode("UTF-8")
        res = node.write(data)
        if isinstance(res, int):
            raise llfuse.FUSEError(res)
        return len(buf)

    def setattr(self, inode, attr):
        return self.getattr(inode)

    def lookup(self, inode_p, name):
        parent = self.inode_to_node_[inode_p]
        name = name.decode('UTF-8')
        log.debug("FS:lookup: {} -> {}".format(inode_p, name))
        try:
            return self._getattr(parent.lookup(name))
        except KeyError:
            raise llfuse.FUSEError(errno.ENOENT)

    def run(self, debug=False):
        # Setup our fuse interaction, but don't process requests yet.
        opts = ['fsname=thingfs', 'nonempty']
        if debug:
            opts.append('debug')
        llfuse.init(self, self.mount_path, opts)
        self.ready_ = True

        try:
            llfuse.main(single=True)
        except:
            llfuse.close(unmount=False)
            raise

        llfuse.close()

