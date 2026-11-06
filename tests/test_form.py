from __future__ import annotations

import json

from up366.form import form_encode


def test_form_encode_replaces_triple_hash_keys_with_dots():
    encoded = form_encode({"pager###pageSize": 20, "plain": "v"})

    assert encoded == {"pager.pageSize": "20", "plain": "v"}


def test_form_encode_serializes_mappings_and_lists_as_json():
    encoded = form_encode(
        {
            "filter": {"name": "词汇", "page": 1},
            "ids": [1, "two", {"three": 3}],
        }
    )

    assert json.loads(encoded["filter"]) == {"name": "词汇", "page": 1}
    assert json.loads(encoded["ids"]) == [1, "two", {"three": 3}]
    assert "词汇" in encoded["filter"]


def test_form_encode_keeps_strings_and_stringifies_scalars():
    encoded = form_encode({"courseId": 9222783, "roleName": "Red"})

    assert encoded == {"courseId": "9222783", "roleName": "Red"}


def test_form_encode_returns_empty_dict_for_empty_input():
    assert form_encode({}) == {}
