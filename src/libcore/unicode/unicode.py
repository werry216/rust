#!/usr/bin/env python

"""
Regenerate Unicode tables (tables.rs).
"""

# This script uses the Unicode tables as defined
# in the UnicodeFiles class.

# Since this should not require frequent updates, we just store this
# out-of-line and check the tables.rs file into git.

# Note that the "curl" program is required for operation.
# This script is compatible with Python 2.7 and 3.x.

import argparse
import datetime
import fileinput
import itertools
import os
import re
import textwrap
import subprocess

from collections import defaultdict, namedtuple

try:
    # Python 3
    from itertools import zip_longest
    from io import StringIO
except ImportError:
    # Python 2 compatibility
    zip_longest = itertools.izip_longest
    from StringIO import StringIO

try:
    # completely optional type hinting
    # (Python 2 compatible using comments,
    #  see: https://mypy.readthedocs.io/en/latest/python2.html)
    # This is very helpful in typing-aware IDE like PyCharm.
    from typing import Any, Callable, Dict, Iterable, Iterator, List, Optional, Set, Tuple
except ImportError:
    pass


# we don't use enum.Enum because of Python 2.7 compatibility
class UnicodeFiles(object):
    # ReadMe does not contain any unicode data, we
    # only use it to extract versions.
    README = "ReadMe.txt"

    DERIVED_CORE_PROPERTIES = "DerivedCoreProperties.txt"
    DERIVED_NORMALIZATION_PROPS = "DerivedNormalizationProps.txt"
    PROPS = "PropList.txt"
    SCRIPTS = "Scripts.txt"
    SPECIAL_CASING = "SpecialCasing.txt"
    UNICODE_DATA = "UnicodeData.txt"


UnicodeFiles.ALL_FILES = tuple(
    getattr(UnicodeFiles, name) for name in dir(UnicodeFiles)
    if not name.startswith("_")
)

# The directory this file is located in.
THIS_DIR = os.path.dirname(os.path.realpath(__file__))

# Where to download the Unicode data.  The downloaded files
# will be placed in sub-directories named after Unicode version.
FETCH_DIR = os.path.join(THIS_DIR, "downloaded")

FETCH_URL_LATEST = "ftp://ftp.unicode.org/Public/UNIDATA/{filename}"
FETCH_URL_VERSION = "ftp://ftp.unicode.org/Public/{version}/ucd/{filename}"

PREAMBLE = """\
// NOTE: The following code was generated by "./unicode.py", do not edit directly

#![allow(missing_docs, non_upper_case_globals, non_snake_case)]

use unicode::version::UnicodeVersion;
use unicode::bool_trie::{{BoolTrie, SmallBoolTrie}};
""".format(year=datetime.datetime.now().year)

# Mapping taken from Table 12 from:
# http://www.unicode.org/reports/tr44/#General_Category_Values
EXPANDED_CATEGORIES = {
    "Lu": ["LC", "L"], "Ll": ["LC", "L"], "Lt": ["LC", "L"],
    "Lm": ["L"], "Lo": ["L"],
    "Mn": ["M"], "Mc": ["M"], "Me": ["M"],
    "Nd": ["N"], "Nl": ["N"], "No": ["N"],
    "Pc": ["P"], "Pd": ["P"], "Ps": ["P"], "Pe": ["P"],
    "Pi": ["P"], "Pf": ["P"], "Po": ["P"],
    "Sm": ["S"], "Sc": ["S"], "Sk": ["S"], "So": ["S"],
    "Zs": ["Z"], "Zl": ["Z"], "Zp": ["Z"],
    "Cc": ["C"], "Cf": ["C"], "Cs": ["C"], "Co": ["C"], "Cn": ["C"],
}

# this is the surrogate codepoints range (both ends inclusive)
# - they are not valid Rust characters
SURROGATE_CODEPOINTS_RANGE = (0xd800, 0xdfff)

UnicodeData = namedtuple(
    "UnicodeData", (
        # conversions:
        "to_upper", "to_lower", "to_title",

        # decompositions: canonical decompositions, compatibility decomp
        "canon_decomp", "compat_decomp",

        # grouped: general categories and combining characters
        "general_categories", "combines",
    )
)

UnicodeVersion = namedtuple(
    "UnicodeVersion", ("major", "minor", "micro", "as_str")
)


def fetch_files(version=None):
    # type: (str) -> UnicodeVersion
    """
    Fetch all the Unicode files from unicode.org.

    This will use cached files (stored in FETCH_DIR) if they exist,
    creating them if they don't.  In any case, the Unicode version
    is always returned.

    :param version: The desired Unicode version, as string.
        (If None, defaults to latest final release available,
         querying the unicode.org service).
    """
    have_version = check_stored_version(version)
    if have_version:
        return have_version

    if version:
        # check if the desired version exists on the server
        get_fetch_url = lambda name: FETCH_URL_VERSION.format(version=version, filename=name)
    else:
        # extract the latest version
        get_fetch_url = lambda name: FETCH_URL_LATEST.format(filename=name)

    readme_url = get_fetch_url(UnicodeFiles.README)

    print("Fetching: {}".format(readme_url))
    readme_content = subprocess.check_output(("curl", readme_url))

    unicode_version = parse_readme_unicode_version(
        readme_content.decode("utf8")
    )

    download_dir = get_unicode_dir(unicode_version)
    if not os.path.exists(download_dir):
        # for 2.7 compat, we don't use exist_ok=True
        os.makedirs(download_dir)

    for filename in UnicodeFiles.ALL_FILES:
        file_path = get_unicode_file_path(unicode_version, filename)

        if os.path.exists(file_path):
            # assume file on the server didn't change if it's been saved before
            continue

        if filename == UnicodeFiles.README:
            with open(file_path, "wb") as fd:
                fd.write(readme_content)
        else:
            url = get_fetch_url(filename)
            print("Fetching: {}".format(url))
            subprocess.check_call(("curl", "-o", file_path, url))

    return unicode_version


def check_stored_version(version):
    # type: (Optional[str]) -> Optional[UnicodeVersion]
    """
    Given desired Unicode version, return the version
    if stored files are all present, and None otherwise.
    """
    if not version:
        # should always check latest version
        return None

    fetch_dir = os.path.join(FETCH_DIR, version)

    for filename in UnicodeFiles.ALL_FILES:
        file_path = os.path.join(fetch_dir, filename)

        if not os.path.exists(file_path):
            return None

    with open(os.path.join(fetch_dir, UnicodeFiles.README)) as fd:
        return parse_readme_unicode_version(fd.read())


def parse_readme_unicode_version(readme_content):
    # type: (str) -> UnicodeVersion
    """
    Parse the Unicode version contained in their ReadMe.txt file.
    """
    # "raw string" is necessary for \d not being treated as escape char
    # (for the sake of compat with future Python versions)
    # see: https://docs.python.org/3.6/whatsnew/3.6.html#deprecated-python-behavior
    pattern = r"for Version (\d+)\.(\d+)\.(\d+) of the Unicode"
    groups = re.search(pattern, readme_content).groups()

    return UnicodeVersion(*map(int, groups), as_str=".".join(groups))


def get_unicode_dir(unicode_version):
    # type: (UnicodeVersion) -> str
    """
    Indicate where the unicode data files should be stored.

    This returns a full, absolute path.
    """
    return os.path.join(FETCH_DIR, unicode_version.as_str)


def get_unicode_file_path(unicode_version, filename):
    # type: (UnicodeVersion, str) -> str
    """
    Indicate where the unicode data file should be stored.
    """
    return os.path.join(get_unicode_dir(unicode_version), filename)


def is_surrogate(n):
    # type: (int) -> bool
    """
    Tell if given codepoint is a surrogate (not a valid Rust character).
    """
    return SURROGATE_CODEPOINTS_RANGE[0] <= n <= SURROGATE_CODEPOINTS_RANGE[1]


def load_unicode_data(file_path):
    # type: (str) -> UnicodeData
    """
    Load main unicode data.
    """
    # conversions
    to_lower = {}   # type: Dict[int, Tuple[int, int, int]]
    to_upper = {}   # type: Dict[int, Tuple[int, int, int]]
    to_title = {}   # type: Dict[int, Tuple[int, int, int]]

    # decompositions
    compat_decomp = {}   # type: Dict[int, List[int]]
    canon_decomp = {}    # type: Dict[int, List[int]]

    # combining characters
    # FIXME: combines are not used
    combines = defaultdict(set)   # type: Dict[str, Set[int]]

    # categories
    general_categories = defaultdict(set)   # type: Dict[str, Set[int]]
    category_assigned_codepoints = set()    # type: Set[int]

    all_codepoints = {}

    range_start = -1

    for line in fileinput.input(file_path):
        data = line.split(";")
        if len(data) != 15:
            continue
        codepoint = int(data[0], 16)
        if is_surrogate(codepoint):
            continue
        if range_start >= 0:
            for i in range(range_start, codepoint):
                all_codepoints[i] = data
            range_start = -1
        if data[1].endswith(", First>"):
            range_start = codepoint
            continue
        all_codepoints[codepoint] = data

    for code, data in all_codepoints.items():
        (code_org, name, gencat, combine, bidi,
         decomp, deci, digit, num, mirror,
         old, iso, upcase, lowcase, titlecase) = data

        # generate char to char direct common and simple conversions
        # uppercase to lowercase
        if lowcase != "" and code_org != lowcase:
            to_lower[code] = (int(lowcase, 16), 0, 0)

        # lowercase to uppercase
        if upcase != "" and code_org != upcase:
            to_upper[code] = (int(upcase, 16), 0, 0)

        # title case
        if titlecase.strip() != "" and code_org != titlecase:
            to_title[code] = (int(titlecase, 16), 0, 0)

        # store decomposition, if given
        if decomp:
            decompositions = decomp.split()[1:]
            decomp_code_points = [int(i, 16) for i in decompositions]

            if decomp.startswith("<"):
                # compatibility decomposition
                compat_decomp[code] = decomp_code_points
            else:
                # canonical decomposition
                canon_decomp[code] = decomp_code_points

        # place letter in categories as appropriate
        for cat in itertools.chain((gencat, ), EXPANDED_CATEGORIES.get(gencat, [])):
            general_categories[cat].add(code)
            category_assigned_codepoints.add(code)

        # record combining class, if any
        if combine != "0":
            combines[combine].add(code)

    # generate Not_Assigned from Assigned
    general_categories["Cn"] = get_unassigned_codepoints(category_assigned_codepoints)

    # Other contains Not_Assigned
    general_categories["C"].update(general_categories["Cn"])

    grouped_categories = group_categories(general_categories)

    # FIXME: combines are not used
    return UnicodeData(
        to_lower=to_lower, to_upper=to_upper, to_title=to_title,
        compat_decomp=compat_decomp, canon_decomp=canon_decomp,
        general_categories=grouped_categories, combines=combines,
    )


def load_special_casing(file_path, unicode_data):
    # type: (str, UnicodeData) -> None
    """
    Load special casing data and enrich given unicode data.
    """
    for line in fileinput.input(file_path):
        data = line.split("#")[0].split(";")
        if len(data) == 5:
            code, lower, title, upper, _comment = data
        elif len(data) == 6:
            code, lower, title, upper, condition, _comment = data
            if condition.strip():  # Only keep unconditional mappins
                continue
        else:
            continue
        code = code.strip()
        lower = lower.strip()
        title = title.strip()
        upper = upper.strip()
        key = int(code, 16)
        for (map_, values) in ((unicode_data.to_lower, lower),
                               (unicode_data.to_upper, upper),
                               (unicode_data.to_title, title)):
            if values != code:
                split = values.split()

                codepoints = list(itertools.chain(
                    (int(i, 16) for i in split),
                    (0 for _ in range(len(split), 3))
                ))

                assert len(codepoints) == 3
                map_[key] = codepoints


def group_categories(mapping):
    # type: (Dict[Any, Iterable[int]]) -> Dict[str, List[Tuple[int, int]]]
    """
    Group codepoints mapped in "categories".
    """
    return {category: group_codepoints(codepoints)
            for category, codepoints in mapping.items()}


def group_codepoints(codepoints):
    # type: (Iterable[int]) -> List[Tuple[int, int]]
    """
    Group integral values into continuous, disjoint value ranges.

    Performs value deduplication.

    :return: sorted list of pairs denoting start and end of codepoint
        group values, both ends inclusive.

    >>> group_codepoints([1, 2, 10, 11, 12, 3, 4])
    [(1, 4), (10, 12)]
    >>> group_codepoints([1])
    [(1, 1)]
    >>> group_codepoints([1, 5, 6])
    [(1, 1), (5, 6)]
    >>> group_codepoints([])
    []
    """
    sorted_codes = sorted(set(codepoints))
    result = []     # type: List[Tuple[int, int]]

    if not sorted_codes:
        return result

    next_codes = sorted_codes[1:]
    start_code = sorted_codes[0]

    for code, next_code in zip_longest(sorted_codes, next_codes, fillvalue=None):
        if next_code is None or next_code - code != 1:
            result.append((start_code, code))
            start_code = next_code

    return result


def ungroup_codepoints(codepoint_pairs):
    # type: (Iterable[Tuple[int, int]]) -> List[int]
    """
    The inverse of group_codepoints -- produce a flat list of values
    from value range pairs.

    >>> ungroup_codepoints([(1, 4), (10, 12)])
    [1, 2, 3, 4, 10, 11, 12]
    >>> ungroup_codepoints([(1, 1), (5, 6)])
    [1, 5, 6]
    >>> ungroup_codepoints(group_codepoints([1, 2, 7, 8]))
    [1, 2, 7, 8]
    >>> ungroup_codepoints([])
    []
    """
    return list(itertools.chain.from_iterable(
        range(lo, hi + 1) for lo, hi in codepoint_pairs
    ))


def get_unassigned_codepoints(assigned_codepoints):
    # type: (Set[int]) -> Set[int]
    """
    Given a set of "assigned" codepoints, return a set
    of these that are not in assigned and not surrogate.
    """
    return {i for i in range(0, 0x110000)
            if i not in assigned_codepoints and not is_surrogate(i)}


def generate_table_lines(items, indent, wrap=98):
    # type: (Iterable[str], int, int) -> Iterator[str]
    """
    Given table items, generate wrapped lines of text with comma-separated items.

    This is a generator function.

    :param wrap: soft wrap limit (characters per line), integer.
    """
    line = " " * indent
    first = True
    for item in items:
        if len(line) + len(item) < wrap:
            if first:
                line += item
            else:
                line += ", " + item
            first = False
        else:
            yield line + ",\n"
            line = " " * indent + item

    yield line


def load_properties(file_path, interesting_props):
    # type: (str, Iterable[str]) -> Dict[str, List[Tuple[int, int]]]
    """
    Load properties data and return in grouped form.
    """
    props = defaultdict(list)   # type: Dict[str, List[Tuple[int, int]]]
    # "raw string" is necessary for \. and \w not to be treated as escape chars
    # (for the sake of compat with future Python versions)
    # see: https://docs.python.org/3.6/whatsnew/3.6.html#deprecated-python-behavior
    re1 = re.compile(r"^ *([0-9A-F]+) *; *(\w+)")
    re2 = re.compile(r"^ *([0-9A-F]+)\.\.([0-9A-F]+) *; *(\w+)")

    for line in fileinput.input(file_path):
        match = re1.match(line) or re2.match(line)
        if match:
            groups = match.groups()

            if len(groups) == 2:
                # re1 matched
                d_lo, prop = groups
                d_hi = d_lo
            else:
                d_lo, d_hi, prop = groups
        else:
            continue

        if interesting_props and prop not in interesting_props:
            continue

        lo_value = int(d_lo, 16)
        hi_value = int(d_hi, 16)

        props[prop].append((lo_value, hi_value))

    # optimize if possible
    for prop in props:
        props[prop] = group_codepoints(ungroup_codepoints(props[prop]))

    return props


def escape_char(c):
    # type: (int) -> str
    r"""
    Escape a codepoint for use as Rust char literal.

    Outputs are OK to use as Rust source code as char literals
    and they also include necessary quotes.

    >>> escape_char(97)
    "'\\u{61}'"
    >>> escape_char(0)
    "'\\0'"
    """
    return r"'\u{%x}'" % c if c != 0 else r"'\0'"


def format_char_pair(pair):
    # type: (Tuple[int, int]) -> str
    """
    Format a pair of two Rust chars.
    """
    return "(%s,%s)" % (escape_char(pair[0]), escape_char(pair[1]))


def generate_table(
    name,   # type: str
    items,  # type: List[Tuple[int, int]]
    decl_type="&[(char, char)]",    # type: str
    is_pub=True,                    # type: bool
    format_item=format_char_pair,   # type: Callable[[Tuple[int, int]], str]
):
    # type: (...) -> Iterator[str]
    """
    Generate a nicely formatted Rust constant "table" array.

    This generates actual Rust code.
    """
    pub_string = ""
    if is_pub:
        pub_string = "pub "

    yield "    %sconst %s: %s = &[\n" % (pub_string, name, decl_type)

    data = []
    first = True
    for item in items:
        if not first:
            data.append(",")
        first = False
        data.extend(format_item(item))

    for table_line in generate_table_lines("".join(data).split(","), 8):
        yield table_line

    yield "\n    ];\n\n"


def compute_trie(raw_data, chunk_size):
    # type: (List[int], int) -> Tuple[List[int], List[int]]
    """
    Compute postfix-compressed trie.

    See: bool_trie.rs for more details.

    >>> compute_trie([1, 2, 3, 1, 2, 3, 4, 5, 6], 3)
    ([0, 0, 1], [1, 2, 3, 4, 5, 6])
    >>> compute_trie([1, 2, 3, 1, 2, 4, 4, 5, 6], 3)
    ([0, 1, 2], [1, 2, 3, 1, 2, 4, 4, 5, 6])
    """
    root = []
    childmap = {}       # type: Dict[Tuple[int, ...], int]
    child_data = []

    assert len(raw_data) % chunk_size == 0, "Chunks must be equally sized"

    for i in range(len(raw_data) // chunk_size):
        data = raw_data[i * chunk_size : (i + 1) * chunk_size]

        # postfix compression of child nodes (data chunks)
        # (identical child nodes are shared)

        # make a tuple out of the list so it's hashable
        child = tuple(data)
        if child not in childmap:
            childmap[child] = len(childmap)
            child_data.extend(data)

        root.append(childmap[child])

    return root, child_data


def generate_bool_trie(name, codepoint_ranges, is_pub=True):
    # type: (str, List[Tuple[int, int]], bool) -> Iterator[str]
    """
    Generate Rust code for BoolTrie struct.

    This yields string fragments that should be joined to produce
    the final string.

    See: bool_trie.rs
    """
    chunk_size = 64
    rawdata = [False] * 0x110000
    for (lo, hi) in codepoint_ranges:
        for cp in range(lo, hi + 1):
            rawdata[cp] = True

    # convert to bitmap chunks of chunk_size bits each
    chunks = []
    for i in range(0x110000 // chunk_size):
        chunk = 0
        for j in range(chunk_size):
            if rawdata[i * chunk_size + j]:
                chunk |= 1 << j
        chunks.append(chunk)

    pub_string = ""
    if is_pub:
        pub_string = "pub "
    yield "    %sconst %s: &super::BoolTrie = &super::BoolTrie {\n" % (pub_string, name)
    yield "        r1: [\n"
    data = ("0x%016x" % chunk for chunk in chunks[:0x800 // chunk_size])
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    # 0x800..0x10000 trie
    (r2, r3) = compute_trie(chunks[0x800 // chunk_size : 0x10000 // chunk_size], 64 // chunk_size)
    yield "        r2: [\n"
    data = map(str, r2)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "        r3: &[\n"
    data = ("0x%016x" % node for node in r3)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    # 0x10000..0x110000 trie
    (mid, r6) = compute_trie(chunks[0x10000 // chunk_size : 0x110000 // chunk_size],
                             64 // chunk_size)
    (r4, r5) = compute_trie(mid, 64)

    yield "        r4: [\n"
    data = map(str, r4)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "        r5: &[\n"
    data = map(str, r5)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "        r6: &[\n"
    data = ("0x%016x" % node for node in r6)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "    };\n\n"


def generate_small_bool_trie(name, codepoint_ranges, is_pub=True):
    # type: (str, List[Tuple[int, int]], bool) -> Iterator[str]
    """
    Generate Rust code for SmallBoolTrie struct.

    See: bool_trie.rs
    """
    last_chunk = max(hi // 64 for (lo, hi) in codepoint_ranges)
    n_chunks = last_chunk + 1
    chunks = [0] * n_chunks
    for (lo, hi) in codepoint_ranges:
        for cp in range(lo, hi + 1):
            assert cp // 64 < len(chunks)
            chunks[cp // 64] |= 1 << (cp & 63)

    pub_string = ""
    if is_pub:
        pub_string = "pub "

    yield ("    %sconst %s: &super::SmallBoolTrie = &super::SmallBoolTrie {\n"
           % (pub_string, name))

    (r1, r2) = compute_trie(chunks, 1)

    yield "        r1: &[\n"
    data = (str(node) for node in r1)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "        r2: &[\n"
    data = ("0x%016x" % node for node in r2)
    for fragment in generate_table_lines(data, 12):
        yield fragment
    yield "\n        ],\n"

    yield "    };\n\n"


def generate_property_module(mod, grouped_categories, category_subset):
    # type: (str, Dict[str, List[Tuple[int, int]]], Iterable[str]) -> Iterator[str]
    """
    Generate Rust code for module defining properties.
    """

    yield "pub mod %s {\n" % mod
    for cat in sorted(category_subset):
        if cat in ("Cc", "White_Space", "Pattern_White_Space"):
            generator = generate_small_bool_trie("%s_table" % cat, grouped_categories[cat])
        else:
            generator = generate_bool_trie("%s_table" % cat, grouped_categories[cat])

        for fragment in generator:
            yield fragment

        yield "    pub fn %s(c: char) -> bool {\n" % cat
        yield "        %s_table.lookup(c)\n" % cat
        yield "    }\n\n"

    yield "}\n\n"


def generate_conversions_module(unicode_data):
    # type: (UnicodeData) -> Iterator[str]
    """
    Generate Rust code for module defining conversions.
    """

    yield "pub mod conversions {"
    yield """
    pub fn to_lower(c: char) -> [char; 3] {
        match bsearch_case_table(c, to_lowercase_table) {
            None        => [c, '\\0', '\\0'],
            Some(index) => to_lowercase_table[index].1,
        }
    }

    pub fn to_upper(c: char) -> [char; 3] {
        match bsearch_case_table(c, to_uppercase_table) {
            None        => [c, '\\0', '\\0'],
            Some(index) => to_uppercase_table[index].1,
        }
    }

    fn bsearch_case_table(c: char, table: &[(char, [char; 3])]) -> Option<usize> {
        table.binary_search_by(|&(key, _)| key.cmp(&c)).ok()
    }\n\n"""

    decl_type = "&[(char, [char; 3])]"
    format_conversion = lambda x: "({},[{},{},{}])".format(*(
        escape_char(c) for c in (x[0], x[1][0], x[1][1], x[1][2])
    ))

    for fragment in generate_table(
        name="to_lowercase_table",
        items=sorted(unicode_data.to_lower.items(), key=lambda x: x[0]),
        decl_type=decl_type,
        is_pub=False,
        format_item=format_conversion
    ):
        yield fragment

    for fragment in generate_table(
        name="to_uppercase_table",
        items=sorted(unicode_data.to_upper.items(), key=lambda x: x[0]),
        decl_type=decl_type,
        is_pub=False,
        format_item=format_conversion
    ):
        yield fragment

    yield "}\n"


def parse_args():
    # type: () -> argparse.Namespace
    """
    Parse command line arguments.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("-v", "--version", default=None, type=str,
                        help="Unicode version to use (if not specified,"
                             " defaults to latest available final release).")

    return parser.parse_args()


def main():
    # type: () -> None
    """
    Script entry point.
    """
    args = parse_args()

    unicode_version = fetch_files(args.version)
    print("Using Unicode version: {}".format(unicode_version.as_str))

    # all the writing happens entirely in memory, we only write to file
    # once we have generated the file content (it's not very large, <1 MB)
    buf = StringIO()
    buf.write(PREAMBLE)

    unicode_version_notice = textwrap.dedent("""
    /// The version of [Unicode](http://www.unicode.org/) that the Unicode parts of
    /// `char` and `str` methods are based on.
    #[unstable(feature = "unicode_version", issue = "49726")]
    pub const UNICODE_VERSION: UnicodeVersion = UnicodeVersion {{
        major: {version.major},
        minor: {version.minor},
        micro: {version.micro},
        _priv: (),
    }};
    """).format(version=unicode_version)
    buf.write(unicode_version_notice)

    get_path = lambda f: get_unicode_file_path(unicode_version, f)

    unicode_data = load_unicode_data(get_path(UnicodeFiles.UNICODE_DATA))
    load_special_casing(get_path(UnicodeFiles.SPECIAL_CASING), unicode_data)

    want_derived = {"XID_Start", "XID_Continue", "Alphabetic", "Lowercase", "Uppercase",
                    "Cased", "Case_Ignorable", "Grapheme_Extend"}
    derived = load_properties(get_path(UnicodeFiles.DERIVED_CORE_PROPERTIES), want_derived)

    props = load_properties(get_path(UnicodeFiles.PROPS),
                            {"White_Space", "Join_Control", "Noncharacter_Code_Point",
                             "Pattern_White_Space"})

    # category tables
    for (name, categories, category_subset) in (
            ("general_category", unicode_data.general_categories, ["N", "Cc"]),
            ("derived_property", derived, want_derived),
            ("property", props, ["White_Space", "Pattern_White_Space"])
    ):
        for fragment in generate_property_module(name, categories, category_subset):
            buf.write(fragment)

    for fragment in generate_conversions_module(unicode_data):
        buf.write(fragment)

    tables_rs_path = os.path.join(THIS_DIR, "tables.rs")

    # will overwrite the file if it exists
    with open(tables_rs_path, "w") as fd:
        fd.write(buf.getvalue())

    print("Regenerated tables.rs.")


if __name__ == "__main__":
    main()
