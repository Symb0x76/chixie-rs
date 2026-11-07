"""Regenerate the synthetic ChiXie3 template fixtures (`*.cx`).

ChiXie3 stores templates as Python pickles (protocol 4) of `{name: NoteTemplate}` and
`{name: CompositionTemplate}` objects from `core.DataModel.*`. These fixtures mimic that layout
with stub classes, so tests never need a real user's files.

    uv run python -I crates/chixie-core/tests/fixtures/make_cx_fixtures.py
"""

import pathlib
import pickle
import sys
import types

HERE = pathlib.Path(__file__).parent


def stub(modname: str, clsname: str) -> type:
    parts = modname.split(".")
    for k in range(1, len(parts) + 1):
        name = ".".join(parts[:k])
        sys.modules.setdefault(name, types.ModuleType(name))
    cls = type(clsname, (object,), {})
    cls.__module__ = modname
    cls.__qualname__ = clsname
    setattr(sys.modules[modname], clsname, cls)
    return cls


def obj(cls: type, **fields):
    o = cls.__new__(cls)
    o.__dict__.update(fields)
    return o


Note = stub("core.DataModel.NoteTemplate", "NoteTemplate")
Comp = stub("core.DataModel.CompositionTemplate", "CompositionTemplate")

NOTE_COMMON = dict(
    enable_dashed_line_partition=True,
    column_width=[],
    column_space=[],
    column_width_equal=True,
    show_dividing_line=True,
    column_same_type=False,
)

notes = {
    "信纸22行": obj(
        Note,
        template_name="信纸22行",
        page_width=160.0,
        page_height=219.8,
        margin_top=1.0,
        margin_bottom=1.0,
        margin_left=1.5,
        margin_right=1.5,
        min_row_num=22,
        max_row_num=22,
        enable_dual_line_mode=False,
        up_row_height=2.5,
        column_num=1,
        **NOTE_COMMON,
    ),
    "随机双栏": obj(
        Note,
        template_name="随机双栏",
        page_width=210.0,
        page_height=297.0,
        margin_top=30,  # ints, as ChiXie3 writes for whole-mm margins
        margin_bottom=20,
        margin_left=20,
        margin_right=20,
        min_row_num=20,
        max_row_num=26,
        enable_dual_line_mode=True,
        up_row_height=2.5,
        column_num=2,
        **{**NOTE_COMMON, "column_space": [8.0]},
    ),
}
grids = {
    "作文格": obj(
        Comp,
        template_name="作文格",
        page_width=190.0,
        page_height=255.0,
        cell_row_number=20,
        cell_column_number=20,
        left_margin=15.0,
        top_margin=20.0,
        cell_width=8.0,
        cell_height=8.0,
        row_spacing=2.9,
        column_spacing=0.0,
    )
}

(HERE / "note_template.cx").write_bytes(pickle.dumps(notes, protocol=4))
(HERE / "composition_template.cx").write_bytes(pickle.dumps(grids, protocol=4))
print("ok")
