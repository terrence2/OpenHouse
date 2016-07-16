#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from enum import IntEnum
from threading import Thread
from oh_shared.args import add_common_args
from oh_shared.db import Connection, Tree
from oh_shared.log import enable_logging
import argparse
import asyncio
import errno
import faulthandler
import llfuse
import logging
import stat


faulthandler.enable()
log = logging.getLogger('oh_fs')


class NodeType(IntEnum):
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
        # We represent stat bitfields like st_mode split out and use properties
        # to combine them for display.
        self.permission_ = 0o600
        self.mode_ = NodeType.Unknown
        self.nlink_ = 2

    @property
    def mode(self):
        assert self.mode_ is not None
        return self.permission_ | self.mode_

    @property
    def nlink(self):
        return self.nlink_

    @property
    def type(self) -> NodeType:
        return self.mode_


class File(Node):
    def __init__(self, read_function: callable, write_function: callable):
        super().__init__()

        # Required overrides.
        self.mode_ = stat.S_IFREG

        # properties
        self.read_function_ = read_function
        self.write_function_ = write_function

    def read(self) -> str:
        if self.read_function_:
            return self.read_function_()
        return ""

    def write(self, data: str):
        if self.write_function_:
            return self.write_function_(data)
        return errno.EPERM


class Directory(Node):
    def __init__(self):
        super().__init__()

        # Required overrides.
        self.mode_ = stat.S_IFDIR

        # The content of this directory.
        self.entries_ = {  # {str: Node}
            '.': self,
        }

    def add_entry(self, name: str, node: Node) -> Node:
        self.entries_[name] = node
        if node.type == NodeType.Directory:
            self.nlink_ += 1
            node.entries_['..'] = self
        return node

    def lookup(self, name: str) -> Node:
        return self.entries_[name]

    def listdir(self) -> [str]:
        return list(self.entries_.keys())


from collections import Counter
Inode = Counter()


class Entry:
    def __init__(self, name: str):
        """
        Create a new file entry.
        """
        entry = llfuse.EntryAttributes()
        entry.st_ino = next(Inode)
        entry.generation = 0
        entry.entry_timeout = 300
        entry.attr_timeout = 300
        entry.st_uid = 1000
        entry.st_gid = 1000
        entry.st_rdev = 0
        entry.st_size = 4096
        entry.st_blksize = 4096
        entry.st_blocks = 1
        entry.st_atime_ns = 0
        entry.st_mtime_ns = 0
        entry.st_ctime_ns = 0
        entry.st_mode = node.mode
        entry.st_nlink = node.nlink

        self.entry = entry


class FileSystem(llfuse.Operations):
    """
    A generic in-memory file-system that gets its layout from the graph of File and Directory nodes that have been
    added to it and its data from making calls on those nodes.
    """
    def __init__(self, mount_path: str, db: Connection, loop):
        super().__init__()

        self.loop = loop
        self.tree = db

        self.ready_ = False  # Signals initialization of the fuse layer.
        self.mount_path = mount_path

        # Map the inodes the system gives us to the relevant nodes.
        self.inode_to_path_ = {1: "/"}  # ino:int => path

    def root(self):
        return self.root_

    def _getattr(self, path: str):
        entries = self.loop.run_until_complete(self.tree.list_directory(path))
        print("ENTRIES: {}".format(entries))
        sys.exit(1)
        path = self.inode_to_path_[inode]
        entry = llfuse.EntryAttributes()
        entry.st_ino = inode
        entry.generation = 0
        entry.entry_timeout = 300
        entry.attr_timeout = 300
        entry.st_uid = 1000
        entry.st_gid = 1000
        entry.st_rdev = 0
        entry.st_size = 4096
        entry.st_blksize = 4096
        entry.st_blocks = 1
        entry.st_atime_ns = 0
        entry.st_mtime_ns = 0
        entry.st_ctime_ns = 0
        entry.st_mode = node.mode
        entry.st_nlink = node.nlink
        return entry

    def getattr(self, inode, ctx):
        log.debug("FS:getattr: {}".format(inode))
        path = self.inode_to_path_[inode]
        return self._getattr(path)

    def opendir(self, inode, ctx):
        assert inode in self.inode_to_path_
        return inode

    def readdir(self, inode, off):
        node = self.inode_to_path_[inode]
        entries = node.listdir()
        for i, name in enumerate(entries[off:], off):
            child = node.lookup(name)
            stat = self._getattr(child)
            yield (name.encode('UTF-8'), stat, i + 1)

    def open(self, inode, flags):
        assert inode in self.inode_to_path_
        return inode

    def read(self, fh, offset, length):
        node = self.inode_to_path_[fh]
        data = node.read()
        if isinstance(data, int):
            raise llfuse.FUSEError(data)
        if isinstance(data, str):
            data = data.encode('UTF-8')
        assert isinstance(data, bytes)
        return data[offset:offset + length]

    def write(self, fh, offset, buf):
        node = self.inode_to_path_[fh]
        data = buf.decode("UTF-8")
        res = node.write(data)
        if isinstance(res, int):
            raise llfuse.FUSEError(res)
        return len(buf)

    def setattr(self, inode, attr):
        return self.getattr(inode)

    def lookup(self, inode_p, name, ctx):
        parent = self.inode_to_path_[inode_p]
        name = name.decode('UTF-8')
        log.debug("FS:lookup: {} -> {}".format(inode_p, name))

        entry = llfuse.EntryAttributes()
        entry.st_ino = inode
        entry.generation = 0
        entry.entry_timeout = 300
        entry.attr_timeout = 300
        entry.st_uid = 1000
        entry.st_gid = 1000
        entry.st_rdev = 0
        entry.st_size = 4096
        entry.st_blksize = 4096
        entry.st_blocks = 1
        entry.st_atime_ns = 0
        entry.st_mtime_ns = 0
        entry.st_ctime_ns = 0
        entry.st_mode = node.mode
        entry.st_nlink = node.nlink
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
            llfuse.main(workers=1)
        except:
            llfuse.close(unmount=False)
            raise

        llfuse.close()

async def main():
    parser = argparse.ArgumentParser(description="Expose the OH DB as a filesystem.")
    add_common_args(parser)
    parser.add_argument("--mountpoint", '-m', type=str, metavar="DIR",
                        help="Where to mount the filesystem.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    tree = await Tree.connect((args.home_address, args.home_port),
                          args.ca_chain, args.certificate, args.private_key)
    th = Thread(target=lambda mp, t, l: FileSystem(mp, t, l).run(True),
                args=(args.mountpoint, tree, asyncio.get_event_loop()))
    th.start()


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
