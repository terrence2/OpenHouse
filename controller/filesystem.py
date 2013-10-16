import errno
import logging
import stat
import sys

from pprint import pprint

import llfuse

log = logging.getLogger('fs')


class FileSystem(llfuse.Operations):
    """
    Export the floorplan as an interactable filesystem.
    """
    def __init__(self, floorplan):
        self.floorplan = floorplan

        # Map the inodes the system gives us to the relevant device.
        self.inode_to_obj_ = {1: self.floorplan}  # ino:int => object
        self.obj_to_inode_ = {id(self.floorplan): 1}

        self.last_inode_ = 1

    def _lookup_or_create_inode(self, obj):
        if id(obj) in self.obj_to_inode_:
            return self.obj_to_inode_[id(obj)]
        self.last_inode_ += 1
        ino = self.last_inode_
        self.inode_to_obj_[ino] = obj
        self.obj_to_inode_[id(obj)] = ino
        return ino

    def _getattr(self, obj):
        entry = llfuse.EntryAttributes()
        entry.st_ino = self._lookup_or_create_inode(obj)
        entry.generation = 0
        entry.entry_timeout = 300
        entry.attr_timeout = 300
        entry.st_uid = 1000
        entry.st_gid = 1000
        entry.st_rdev = 0
        entry.st_size = 4096
        entry.st_blksize = 4096
        entry.st_blocks = 1
        entry.st_atime = 0
        entry.st_mtime = 0
        entry.st_ctime = 0
        if not obj.is_dir():
            entry.st_mode = 0o600 | stat.S_IFREG
            entry.st_nlink = 1
        else:
            entry.st_mode = 0o600 | stat.S_IFDIR
            entry.st_nlink = 2
        return entry

    def getattr(self, inode):
        obj = self.inode_to_obj_[inode]
        return self._getattr(obj)

    def opendir(self, inode):
        assert inode in self.inode_to_obj_
        return inode

    def readdir(self, inode, off):
        obj = self.inode_to_obj_[inode]
        entries = obj.listdir()
        for i, name in enumerate(entries[off:], off):
            child = obj.lookup(name)
            stat = self._getattr(child)
            yield (name.encode('UTF-8'), stat, i + 1)

    def open(self, inode, flags):
        assert inode in self.inode_to_obj_
        return inode

    def read(self, fh, offset, length):
        obj = self.inode_to_obj_[fh]
        data = obj.read()
        if isinstance(data, int):
            raise llfuse.FUSEError(data)
        data = data.encode('UTF-8')
        return data[offset:offset + length]

    def write(self, fh, offset, buf):
        obj = self.inode_to_obj_[fh]
        data = buf.decode("UTF-8")
        obj.write(data)
        return len(buf)

    def setattr(self, inode, attr):
        return self.getattr(inode)

    def lookup(self, inode_p, name):
        parent = self.inode_to_obj_[inode_p]
        name = name.decode('UTF-8')
        if name == '.':
            return inode_p
        if name == '..':
            return self._lookup_or_ceate_inode(parent.parent())
        try:
            return self._getattr(parent.lookup(name))
        except KeyError:
            raise llfuse.FUSEError(errno.ENOENT)

    def run(self):
        llfuse.init(self, '/things', ['fsname=thingfs', 'nonempty', 'debug'])

        try:
            llfuse.main(single=True)
        except:
            llfuse.close(unmount=False)
            raise

        llfuse.close()


class File:
    def __init__(self, read, write):
        self.read = read
        self.write = write

    def is_dir(self):
        return False

    def read(self):
        data = self.read()
        return data


class Dir:
    def __init__(self, parent):
        self.parent = parent

    def is_dir(self):
        return True

    def listdir(self):
        raise llfuse.FUSEError(errno.ENOTSUP)

