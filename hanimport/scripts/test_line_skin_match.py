from line_skin_match import (
    apply_lines_by_skin,
    match_wiki_group_to_skin,
    normalize_skin_label,
)


def test_normalize_skin_label():
    assert normalize_skin_label("换装「待宵的魔女」") == normalize_skin_label("待宵的魔女")
    assert "誓约" not in normalize_skin_label("【誓约】鬼神之华裳") or True
    assert normalize_skin_label("【誓约】鬼神之华裳") == normalize_skin_label("鬼神之华裳")


def test_match_default():
    skins = [
        {"id": "edu-default", "name_zh": "默认", "is_default": True},
        {"id": "edu-skin1", "name_zh": "某换装", "is_default": False},
    ]
    sk, how = match_wiki_group_to_skin(
        {"skin": "default", "skin_kind": "default", "lines": []}, skins
    )
    assert sk["id"] == "edu-default"
    assert how == "default"


def test_match_by_name_zh():
    skins = [
        {"id": "a-default", "name_zh": "默认", "is_default": True},
        {"id": "a-witch", "name_zh": "待宵的魔女", "is_default": False, "kanmusu_dir": ""},
    ]
    sk, how = match_wiki_group_to_skin(
        {"skin": "待宵的魔女", "skin_kind": "skin", "lines": [{"key": "login", "text": "x"}]},
        skins,
    )
    assert sk["id"] == "a-witch"
    assert how == "name_zh"


def test_unmatched_returns_none():
    skins = [{"id": "a-default", "name_zh": "默认", "is_default": True}]
    sk, how = match_wiki_group_to_skin(
        {"skin": "完全不存在的皮肤名", "skin_kind": "skin", "lines": []},
        skins,
    )
    assert sk is None and how is None


def test_apply_lines_report():
    skins = [
        {"id": "c-default", "name_zh": "默认", "is_default": True},
        {"id": "c-s1", "name_zh": "乐队型鬼神", "is_default": False},
        {"id": "c-orphan", "name_zh": "库里有Wiki没有", "is_default": False},
    ]
    groups = [
        {
            "skin": "default",
            "skin_kind": "default",
            "lines": [{"key": "login", "text": "默认登录", "lang": "zh"}],
        },
        {
            "skin": "乐队型鬼神",
            "skin_kind": "skin",
            "lines": [{"key": "login", "text": "换装登录", "lang": "zh"}],
        },
        {
            "skin": "Wiki独有",
            "skin_kind": "skin",
            "lines": [{"key": "login", "text": "无人认领", "lang": "zh"}],
        },
    ]

    def rows(raw):
        return [
            {
                "wiki_key": x.get("key") or "",
                "label": "",
                "lang": x.get("lang") or "",
                "text": x.get("text") or "",
                "animation": "",
                "audio_url": "",
                "audio_relpath": "",
                "sort_order": i,
            }
            for i, x in enumerate(raw or [])
            if (x.get("text") or "").strip()
        ]

    report = apply_lines_by_skin(groups, skins, rows)
    assert len(report["assignments"]) == 2
    assert report["assignments"][0]["lines"][0]["text"] == "默认登录"
    assert any(a["wiki_skin"] == "乐队型鬼神" for a in report["assignments"])
    assert any(u["skin"] == "Wiki独有" for u in report["wiki_unmatched"])
    assert "c-orphan" in report["roster_unmatched_ids"]
