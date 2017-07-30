import os
from subprocess import check_call
from contextlib import contextmanager


def replace_file(name: str, content: str):
    with open(name, "w") as fp:
        fp.truncate(0)
        fp.write(content)


@contextmanager
def change_directory(path: str):
    current = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(current)


def make_directory(component: str, permissions: int = 0o700):
    assert "/" not in component
    if not os.path.exists(component):
        os.mkdir(component, permissions)
    assert os.path.isdir(component)


def call(command: str, **params: dict):
    check_call(command.format(**params).split())

