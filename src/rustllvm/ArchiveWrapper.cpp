// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#include "rustllvm.h"

#include "llvm/Object/Archive.h"

#if LLVM_VERSION_MINOR >= 7
#include "llvm/Object/ArchiveWriter.h"
#endif

using namespace llvm;
using namespace llvm::object;

struct LLVMRustArchiveMember {
  const char *filename;
  const char *name;
  Archive::Child child;

  LLVMRustArchiveMember(): filename(NULL), name(NULL), child(NULL, NULL) {}
  ~LLVMRustArchiveMember() {}
};

#if LLVM_VERSION_MINOR >= 6
typedef OwningBinary<Archive> RustArchive;
#define GET_ARCHIVE(a) ((a)->getBinary())
#else
typedef Archive RustArchive;
#define GET_ARCHIVE(a) (a)
#endif

extern "C" void*
LLVMRustOpenArchive(char *path) {
    ErrorOr<std::unique_ptr<MemoryBuffer>> buf_or = MemoryBuffer::getFile(path,
                                                                          -1,
                                                                          false);
    if (!buf_or) {
        LLVMRustSetLastError(buf_or.getError().message().c_str());
        return nullptr;
    }

#if LLVM_VERSION_MINOR >= 6
    ErrorOr<std::unique_ptr<Archive>> archive_or =
        Archive::create(buf_or.get()->getMemBufferRef());

    if (!archive_or) {
        LLVMRustSetLastError(archive_or.getError().message().c_str());
        return nullptr;
    }

    OwningBinary<Archive> *ret = new OwningBinary<Archive>(
            std::move(archive_or.get()), std::move(buf_or.get()));
#else
    std::error_code err;
    Archive *ret = new Archive(std::move(buf_or.get()), err);
    if (err) {
        LLVMRustSetLastError(err.message().c_str());
        return nullptr;
    }
#endif

    return ret;
}

extern "C" void
LLVMRustDestroyArchive(RustArchive *ar) {
    delete ar;
}

struct RustArchiveIterator {
    Archive::child_iterator cur;
    Archive::child_iterator end;
};

extern "C" RustArchiveIterator*
LLVMRustArchiveIteratorNew(RustArchive *ra) {
    Archive *ar = GET_ARCHIVE(ra);
    RustArchiveIterator *rai = new RustArchiveIterator();
    rai->cur = ar->child_begin();
    rai->end = ar->child_end();
    return rai;
}

extern "C" const Archive::Child*
LLVMRustArchiveIteratorNext(RustArchiveIterator *rai) {
    if (rai->cur == rai->end)
        return NULL;
    const Archive::Child *cur = rai->cur.operator->();
    Archive::Child *ret = new Archive::Child(*cur);
    ++rai->cur;
    return ret;
}

extern "C" void
LLVMRustArchiveChildFree(Archive::Child *child) {
    delete child;
}

extern "C" void
LLVMRustArchiveIteratorFree(RustArchiveIterator *rai) {
    delete rai;
}

extern "C" const char*
LLVMRustArchiveChildName(const Archive::Child *child, size_t *size) {
    ErrorOr<StringRef> name_or_err = child->getName();
    if (name_or_err.getError())
        return NULL;
    StringRef name = name_or_err.get();
    *size = name.size();
    return name.data();
}

extern "C" const char*
LLVMRustArchiveChildData(Archive::Child *child, size_t *size) {
    StringRef buf;
#if LLVM_VERSION_MINOR >= 7
    ErrorOr<StringRef> buf_or_err = child->getBuffer();
    if (buf_or_err.getError()) {
      LLVMRustSetLastError(buf_or_err.getError().message().c_str());
      return NULL;
    }
    buf = buf_or_err.get();
#else
    buf = child->getBuffer();
#endif
    *size = buf.size();
    return buf.data();
}

extern "C" LLVMRustArchiveMember*
LLVMRustArchiveMemberNew(char *Filename, char *Name, Archive::Child *child) {
    LLVMRustArchiveMember *Member = new LLVMRustArchiveMember;
    Member->filename = Filename;
    Member->name = Name;
    if (child)
        Member->child = *child;
    return Member;
}

extern "C" void
LLVMRustArchiveMemberFree(LLVMRustArchiveMember *Member) {
    delete Member;
}

extern "C" int
LLVMRustWriteArchive(char *Dst,
                     size_t NumMembers,
                     const LLVMRustArchiveMember **NewMembers,
                     bool WriteSymbtab,
                     Archive::Kind Kind) {
#if LLVM_VERSION_MINOR >= 7
  std::vector<NewArchiveIterator> Members;

  for (size_t i = 0; i < NumMembers; i++) {
    auto Member = NewMembers[i];
    assert(Member->name);
    if (Member->filename) {
      Members.push_back(NewArchiveIterator(Member->filename, Member->name));
    } else {
      Members.push_back(NewArchiveIterator(Member->child, Member->name));
    }
  }
  auto pair = writeArchive(Dst, Members, WriteSymbtab, Kind, false);
  if (!pair.second)
    return 0;
  LLVMRustSetLastError(pair.second.message().c_str());
#else
  LLVMRustSetLastError("writing archives not supported with this LLVM version");
#endif
  return -1;
}
