import hegel
from hegel import _native
from hegel._labels import combine_labels, label_from_name


def test_label_from_name_matches_engine():
    for name in ["", "hegel.integers", "ünïcode", "a" * 100]:
        assert label_from_name(name) == _native.label_from_name(name)


def test_combine_labels_matches_engine():
    labels = [label_from_name(n) for n in ["a", "b", "c"]]
    assert combine_labels(labels) == _native.label_combine(labels)
    assert combine_labels([]) == _native.label_combine([])
    assert combine_labels(labels[:1]) != combine_labels(labels[:2])


def test_engine_version():
    assert hegel.engine_version() == _native.version()
    assert hegel.__version__
