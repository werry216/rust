//! This module contains paths to types and functions Clippy needs to know
//! about.

pub const ANY_TRAIT: [&'static str; 3] = ["std", "any", "Any"];
pub const ARC: [&'static str; 3] = ["alloc", "arc", "Arc"];
pub const ASMUT_TRAIT: [&'static str; 3] = ["core", "convert", "AsMut"];
pub const ASREF_TRAIT: [&'static str; 3] = ["core", "convert", "AsRef"];
pub const BEGIN_PANIC: [&'static str; 3] = ["std", "panicking", "begin_panic"];
pub const BINARY_HEAP: [&'static str; 3] = ["alloc", "binary_heap", "BinaryHeap"];
pub const BORROW_TRAIT: [&'static str; 3] = ["core", "borrow", "Borrow"];
pub const BOX: [&'static str; 3] = ["std", "boxed", "Box"];
pub const BOX_NEW: [&'static str; 4] = ["std", "boxed", "Box", "new"];
pub const BTREEMAP: [&'static str; 4] = ["alloc", "btree", "map", "BTreeMap"];
pub const BTREEMAP_ENTRY: [&'static str; 4] = ["alloc", "btree", "map", "Entry"];
pub const BTREESET: [&'static str; 4] = ["alloc", "btree", "set", "BTreeSet"];
pub const CLONE: [&'static str; 4] = ["core", "clone", "Clone", "clone"];
pub const CLONE_TRAIT: [&'static str; 3] = ["core", "clone", "Clone"];
pub const CMP_MAX: [&'static str; 3] = ["core", "cmp", "max"];
pub const CMP_MIN: [&'static str; 3] = ["core", "cmp", "min"];
pub const COW: [&'static str; 3] = ["alloc", "borrow", "Cow"];
pub const CSTRING_NEW: [&'static str; 5] = ["std", "ffi", "c_str", "CString", "new"];
pub const DEBUG_FMT_METHOD: [&'static str; 4] = ["core", "fmt", "Debug", "fmt"];
pub const DEFAULT_TRAIT: [&'static str; 3] = ["core", "default", "Default"];
pub const DISPLAY_FMT_METHOD: [&'static str; 4] = ["core", "fmt", "Display", "fmt"];
pub const DOUBLE_ENDED_ITERATOR: [&'static str; 4] = ["core", "iter", "traits", "DoubleEndedIterator"];
pub const DROP: [&'static str; 3] = ["core", "mem", "drop"];
pub const FMT_ARGUMENTS_NEWV1: [&'static str; 4] = ["core", "fmt", "Arguments", "new_v1"];
pub const FMT_ARGUMENTV1_NEW: [&'static str; 4] = ["core", "fmt", "ArgumentV1", "new"];
pub const HASH: [&'static str; 2] = ["hash", "Hash"];
pub const HASHMAP: [&'static str; 5] = ["std", "collections", "hash", "map", "HashMap"];
pub const HASHMAP_ENTRY: [&'static str; 5] = ["std", "collections", "hash", "map", "Entry"];
pub const HASHSET: [&'static str; 5] = ["std", "collections", "hash", "set", "HashSet"];
pub const INIT: [&'static str; 4] = ["core", "intrinsics", "", "init"];
pub const INTO_ITERATOR: [&'static str; 4] = ["core", "iter", "traits", "IntoIterator"];
pub const IO_PRINT: [&'static str; 4] = ["std", "io", "stdio", "_print"];
pub const IO_READ: [&'static str; 3] = ["std", "io", "Read"];
pub const IO_WRITE: [&'static str; 3] = ["std", "io", "Write"];
pub const ITERATOR: [&'static str; 4] = ["core", "iter", "iterator", "Iterator"];
pub const LINKED_LIST: [&'static str; 3] = ["alloc", "linked_list", "LinkedList"];
pub const LINT: [&'static str; 3] = ["rustc", "lint", "Lint"];
pub const LINT_ARRAY: [&'static str; 3] = ["rustc", "lint", "LintArray"];
pub const MEM_FORGET: [&'static str; 3] = ["core", "mem", "forget"];
pub const MEM_UNINIT: [&'static str; 3] = ["core", "mem", "uninitialized"];
pub const MEM_ZEROED: [&'static str; 3] = ["core", "mem", "zeroed"];
pub const MUTEX: [&'static str; 4] = ["std", "sync", "mutex", "Mutex"];
pub const OPEN_OPTIONS: [&'static str; 3] = ["std", "fs", "OpenOptions"];
pub const OPS_MODULE: [&'static str; 2] = ["core", "ops"];
pub const OPTION: [&'static str; 3] = ["core", "option", "Option"];
pub const OPTION_NONE: [&'static str; 4] = ["core", "option", "Option", "None"];
pub const OPTION_SOME: [&'static str; 4] = ["core", "option", "Option", "Some"];
pub const PTR_NULL: [&'static str; 2] = ["ptr", "null"];
pub const PTR_NULL_MUT: [&'static str; 2] = ["ptr", "null_mut"];
pub const RANGE: [&'static str; 3] = ["core", "ops", "Range"];
pub const RANGE_FROM: [&'static str; 3] = ["core", "ops", "RangeFrom"];
pub const RANGE_FROM_STD: [&'static str; 3] = ["std", "ops", "RangeFrom"];
pub const RANGE_FULL: [&'static str; 3] = ["core", "ops", "RangeFull"];
pub const RANGE_FULL_STD: [&'static str; 3] = ["std", "ops", "RangeFull"];
pub const RANGE_INCLUSIVE: [&'static str; 3] = ["core", "ops", "RangeInclusive"];
pub const RANGE_INCLUSIVE_STD: [&'static str; 3] = ["std", "ops", "RangeInclusive"];
pub const RANGE_STD: [&'static str; 3] = ["std", "ops", "Range"];
pub const RANGE_TO: [&'static str; 3] = ["core", "ops", "RangeTo"];
pub const RANGE_TO_INCLUSIVE: [&'static str; 3] = ["core", "ops", "RangeToInclusive"];
pub const RANGE_TO_INCLUSIVE_STD: [&'static str; 3] = ["std", "ops", "RangeToInclusive"];
pub const RANGE_TO_STD: [&'static str; 3] = ["std", "ops", "RangeTo"];
pub const RC: [&'static str; 3] = ["alloc", "rc", "Rc"];
pub const REGEX: [&'static str; 3] = ["regex", "re_unicode", "Regex"];
pub const REGEX_BUILDER_NEW: [&'static str; 5] = ["regex", "re_builder", "unicode", "RegexBuilder", "new"];
pub const REGEX_BYTES: [&'static str; 3] = ["regex", "re_bytes", "Regex"];
pub const REGEX_BYTES_BUILDER_NEW: [&'static str; 5] = ["regex", "re_builder", "bytes", "RegexBuilder", "new"];
pub const REGEX_BYTES_NEW: [&'static str; 4] = ["regex", "re_bytes", "Regex", "new"];
pub const REGEX_BYTES_SET_NEW: [&'static str; 5] = ["regex", "re_set", "bytes", "RegexSet", "new"];
pub const REGEX_NEW: [&'static str; 4] = ["regex", "re_unicode", "Regex", "new"];
pub const REGEX_SET_NEW: [&'static str; 5] = ["regex", "re_set", "unicode", "RegexSet", "new"];
pub const REPEAT: [&'static str; 3] = ["core", "iter", "repeat"];
pub const RESULT: [&'static str; 3] = ["core", "result", "Result"];
pub const RESULT_ERR: [&'static str; 4] = ["core", "result", "Result", "Err"];
pub const RESULT_OK: [&'static str; 4] = ["core", "result", "Result", "Ok"];
pub const SERDE_DE_VISITOR: [&'static str; 3] = ["serde", "de", "Visitor"];
pub const SLICE_INTO_VEC: [&'static str; 4] = ["alloc", "slice", "<impl [T]>", "into_vec"];
pub const SLICE_ITER: [&str; 3] = ["core", "slice", "Iter"];
pub const STRING: [&'static str; 3] = ["alloc", "string", "String"];
pub const TO_OWNED: [&'static str; 3] = ["alloc", "borrow", "ToOwned"];
pub const TO_STRING: [&'static str; 3] = ["alloc", "string", "ToString"];
pub const TRANSMUTE: [&'static str; 4] = ["core", "intrinsics", "", "transmute"];
pub const TRY_INTO_RESULT: [&'static str; 4] = ["std", "ops", "Try", "into_result"];
pub const UNINIT: [&'static str; 4] = ["core", "intrinsics", "", "uninit"];
pub const VEC: [&'static str; 3] = ["alloc", "vec", "Vec"];
pub const VEC_DEQUE: [&'static str; 3] = ["alloc", "vec_deque", "VecDeque"];
pub const VEC_FROM_ELEM: [&'static str; 3] = ["alloc", "vec", "from_elem"];
pub const WEAK_ARC: [&'static str; 3] = ["alloc", "arc", "Weak"];
pub const WEAK_RC: [&'static str; 3] = ["alloc", "rc", "Weak"];
