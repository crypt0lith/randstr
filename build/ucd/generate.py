import io
import json
import os.path as osp
import re
import sys
import typing as ty
from collections import defaultdict
from enum import IntEnum, auto

import urllib.request

type CodepointRangesDict = dict[str, list[tuple[int, int]]]
type PropertyAliasTuple = tuple[str, str, list[str]]
type PropValueAliasTuple = tuple[int | None, str, str, list[str]]

DERIVE_PATTERN = re.compile(
    r"^([A-Fa-f0-9]{4,5})(?:\.{2}([A-Fa-f0-9]{4,5}))?\s*;\s([^#]*)"
)
SEMI_SEP_PATTERN = re.compile(r"^([^#]+?;[^#]*)+")
PROP_ALIASES: list[PropertyAliasTuple] = []


def parse_derive_data(rf: ty.TextIO):
    d = defaultdict[str, list[tuple[int, int]]](list)
    for line in rf:
        if m := DERIVE_PATTERN.match(line):
            i, j, key = m.group(1, 2, 3)
            if j is None:
                j = i
            d[key.strip()].append((int(i, 16), int(j, 16)))
    for xs in d.values():
        xs.sort()
    return {**d}


def parse_property_aliases(rf: ty.TextIO):
    global PROP_ALIASES
    for line in rf:
        if m := SEMI_SEP_PATTERN.match(line):
            short, long, *rest = map(str.strip, m.group(0).split(";"))
            PROP_ALIASES.append((short, long, rest))


def get_property_index(name: str) -> int:
    try:
        return next(i for i, names in enumerate(PROP_ALIASES) if name in names)
    except StopIteration:
        pass
    raise ValueError(name)


def parse_property_value_aliases(rf: ty.TextIO) -> dict[int, list[PropValueAliasTuple]]:
    d = defaultdict(list)
    for line in rf:
        if m := SEMI_SEP_PATTERN.match(line):
            propname, *value_aliases = map(str.strip, m.group(0).split(";"))
            idx = get_property_index(propname)
            if propname == "ccc":
                numeric, short, long = value_aliases
                d[idx].append((int(numeric), short, long, []))
            else:
                short, long, *rest = value_aliases
                d[idx].append((None, short, long, rest))
    return {**d}


def main():
    import argparse

    arg_parser = argparse.ArgumentParser()
    arg_parser.add_argument(
        dest="rustfile", type=argparse.FileType("w"), nargs="?", default=sys.stdout
    )
    rustfile = arg_parser.parse_args().rustfile

    class UCDPropertyType(IntEnum):
        NONE = 0
        CATALOG = auto()
        ENUMERATION = auto()
        BINARY = auto()
        STRINGVALUE = auto()
        NUMERIC = auto()
        MISC = auto()

    def urlproc[_R](__func: ty.Callable[[ty.TextIO], _R], /, url: str) -> _R:
        with (
            urllib.request.urlopen(url) as resp,
            io.TextIOWrapper(
                resp, encoding=resp.headers.get_content_charset() or "utf-8"
            ) as f,
        ):
            return __func(f)

    prop_alias_map: dict[str, int] = {}
    idx2names_map: dict[int, tuple[str, ...]] = {}
    prop_v_alias_map: dict[int, list[PropValueAliasTuple]] = {}

    codepoint_ranges: dict[int | tuple[int, int], list[tuple[int, int]]] = {}

    split_wordbreaks = re.compile("[ -]").split
    norm_wordbreaks: ty.Callable[[str], str] = lambda s: str.translate(
        "_".join(
            x[0].upper() + ("" if len(x) == 1 else x[1:]) for x in split_wordbreaks(s)
        ),
        {ord(";"): None},
    )

    with open(osp.join(osp.dirname(__file__), "properties.json"), "r") as f:
        remotefiles: tuple[str, str, dict[str, dict[str, list[str]]]] = tuple(
            json.load(f)
        )

    for parser, prop_ctx in zip(
        (
            parse_property_aliases,
            parse_property_value_aliases,
            parse_derive_data,
        ),
        remotefiles,
    ):
        if isinstance(prop_ctx, str):
            ucd_url = f"http://www.unicode.org/Public/UCD/latest/ucd/{prop_ctx}.txt"
            if parser is parse_property_aliases:
                urlproc(parse_property_aliases, ucd_url)
                for i, xs in enumerate(PROP_ALIASES):
                    short, long, rest = xs
                    prop_alias_map[short] = prop_alias_map[long] = i
                    for s in rest:
                        prop_alias_map[s] = i
                    idx2names_map[i] = short, long, *rest
            else:
                prop_v_alias_map |= urlproc(parse_property_value_aliases, ucd_url)
            continue
        for relpath, names in prop_ctx.items():
            ucd_url = f"http://www.unicode.org/Public/UCD/latest/ucd/{relpath}.txt"
            extracted_ranges: dict[str, list[tuple[int, int]]] = urlproc(
                parse_derive_data, ucd_url
            )
            for prop_kind_name, long_names in names.items():
                prop_kind = UCDPropertyType[prop_kind_name]
                for long in long_names:
                    prop_idx = prop_alias_map[long]
                    short = idx2names_map[prop_idx][0]
                    current_v_aliases = prop_v_alias_map[prop_idx]
                    match prop_kind:
                        case UCDPropertyType.ENUMERATION if (
                            long == "Canonical_Combining_Class"
                        ):
                            current_v_indices = {
                                k: i
                                for i, k in enumerate(
                                    str(v[0]) for v in current_v_aliases
                                )
                                if k in extracted_ranges
                            }
                        case UCDPropertyType.ENUMERATION:
                            name_map = {
                                name: i
                                for i, (*_, name, _) in enumerate(current_v_aliases)
                            }
                            current_v_indices = {}
                            for name in extracted_ranges:
                                if name in name_map:
                                    current_v_indices[name] = name_map[name]
                                elif name.startswith(prefix := f"{short};"):
                                    current_v_indices[name] = name_map[
                                        name.removeprefix(prefix).lstrip()
                                    ]
                        case UCDPropertyType.CATALOG:
                            name_map = {
                                name: i
                                for i, (_, _, name, _) in enumerate(current_v_aliases)
                            }
                            current_v_indices = {
                                k: name_map[norm_wordbreaks(k)]
                                for k in extracted_ranges
                            }
                        case UCDPropertyType.NUMERIC:
                            num_map = {
                                str(n): i for i, (n, *_) in enumerate(current_v_aliases)
                            }
                            current_v_indices = {
                                k: num_map[k] for k in extracted_ranges
                            }
                        case UCDPropertyType.BINARY:
                            current_v_indices = None
                        case UCDPropertyType.NONE:
                            abbrev_map = {
                                x[1]: i for i, x in enumerate(current_v_aliases)
                            }
                            current_v_indices = {
                                k: abbrev_map[k] for k in extracted_ranges
                            }
                        case _:
                            raise ValueError
                    if current_v_indices is None:
                        codepoint_ranges[prop_idx] = extracted_ranges[long]
                        continue
                    for k in extracted_ranges.keys() & current_v_indices.keys():
                        value_idx = current_v_indices[k]
                        codepoint_ranges[prop_idx, value_idx] = extracted_ranges[k]

    def add_gc_supersets():
        try:
            gc_idx = prop_alias_map["gc"]
        except KeyError:
            gc_idx = prop_alias_map["General_Category"]
        short2idx = {xs[1]: i for i, xs in enumerate(prop_v_alias_map[gc_idx])}

        def shrink(ranges: list[tuple[int, int]]):
            if not ranges:
                return
            ranges.sort()
            out = []
            lo, hi = ranges[0]
            for a, b in ranges[1:]:
                if a <= hi + 1:
                    hi = max(hi, b)
                else:
                    out.append((lo, hi))
                    lo, hi = a, b
            out.append((lo, hi))
            ranges[:] = out

        for (short, long), members in {
            ("L", "Letter"): "lmotu",
            ("LC", "Cased_Letter"): "ltu",
            ("M", "Mark"): "cen",
            ("N", "Number"): "dlo",
            ("P", "Punctuation"): "cdefios",
            ("S", "Symbol"): "ckmo",
            ("Z", "Separator"): "lsp",
            ("C", "Other"): "cfnos",
        }.items():
            acc = []
            for member in map(short[0].__add__, members):
                k = gc_idx, short2idx[member]
                acc.extend(codepoint_ranges.get(k, ()))
            shrink(acc)
            if not acc:
                continue
            codepoint_ranges[gc_idx, len(prop_v_alias_map[gc_idx])] = acc
            prop_v_alias_map[gc_idx].append((None, short, long, []))

    add_gc_supersets()

    out_prop_groups_map = defaultdict[str, dict[str, str]](dict)
    fmt_ident = (
        lambda s, prefix="PROP": f"{prefix.upper()}_{norm_wordbreaks(s).upper()}"
    )

    def gen_outlines():
        def gen_char_ranges(__ident: str, bounds: list[tuple[int, int]]):
            yield f"pub const {__ident}: CharRanges = &["
            for bound in bounds:
                yield r"    ('\u{%X}', '\u{%X}')," % bound
            yield "];"

        def gen_static_map(
            __ident: str,
            members: ty.Iterable[tuple[str, str]],
            value_type="CharRanges<'static>",
        ):
            [*members] = members
            maxlen = max(len(x[0]) for x in members)
            yield f"pub static {__ident}: phf::Map<&'static str, {value_type}> = phf_map! {{"
            for key, value in members:
                yield f'    {('"%s"' % key): <{maxlen + 2}} => {value},'
            yield "};"

        yield f"// Auto-generated by {osp.basename(__file__)}"
        yield "use phf::phf_map;"
        for ix, ranges in codepoint_ranges.items():
            ranges[:] = sorted(
                (lo, hi) for lo, hi in ranges if not (0xD800 <= lo and hi <= 0xDFFF)
            )
            if not ranges:
                continue
            if isinstance(ix, int):
                ix_names = idx2names_map[ix]
                ident = fmt_ident(ix_names[1])
                for ix_name in map(str.casefold, ix_names):
                    out_prop_groups_map[""][ix_name] = ident
            else:
                ix_names = idx2names_map[ix[0]]
                (n, short, long, rest) = prop_v_alias_map[ix[0]][ix[1]]
                ident = fmt_ident(f"{ix_names[0]}_{(long if n is None else str(n))}")
                if n is not None:
                    rest.append(str(n))
                out_prop_groups_map[ix_names[1].casefold()] |= dict.fromkeys(
                    map(str.casefold, [short, long, *rest]), ident
                )
            yield from gen_char_ranges(ident, ranges)
        for cat_name, member_map in out_prop_groups_map.items():
            yield from gen_static_map(
                fmt_ident(cat_name or str(None), prefix="CAT"),
                member_map.items(),
            )
        yield from gen_static_map(
            "CATEGORY_NAMES",
            {
                (ix_name.casefold(), '&' + fmt_ident(ix_names[1], prefix="CAT"))
                for ix_names in idx2names_map.values()
                if ix_names[1].casefold() in out_prop_groups_map
                for ix_name in ix_names
            },
            value_type="&'static phf::Map<&'static str, CharRanges<'static>>",
        )
        yield from gen_static_map(
            "BINARY_NAMES",
            {
                (ix_name.casefold(), fmt_ident(idx2names_map[ix][1]))
                for ix in codepoint_ranges
                if isinstance(ix, int)
                for ix_name in idx2names_map[ix]
            },
        )

    print(*gen_outlines(), sep="\n", file=rustfile)


if __name__ == "__main__":
    sys.exit(main())
