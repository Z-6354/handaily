"""TDD: unpack folder → character base + skin suffix."""
from __future__ import annotations

from roster_db import strip_skin


def test_strip_default():
    assert strip_skin("z23") == ("z23", "")


def test_strip_digit_skin():
    assert strip_skin("abeikelongbi_3") == ("abeikelongbi", "3")
    assert strip_skin("qiye_9") == ("qiye", "9")


def test_strip_hx_variant():
    assert strip_skin("abeikelongbi_3_hx") == ("abeikelongbi", "3_hx")
    assert strip_skin("z23_hx") == ("z23", "hx")


def test_strip_multi_part_live2d():
    assert strip_skin("abeikelongbi_3_1") == ("abeikelongbi", "3_1")
    assert strip_skin("aidang_2_1") == ("aidang", "2_1")


def test_strip_named_variant():
    assert strip_skin("ship_wedding") == ("ship", "wedding")


def test_strip_doa_collab():
    assert strip_skin("maliluosi_3_doa") == ("maliluosi", "3_doa")
