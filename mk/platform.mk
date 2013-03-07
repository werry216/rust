# Copyright 2012 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.


# Create variables HOST_<triple> containing the host part
# of each target triple.  For example, the triple i686-darwin-macos
# would create a variable HOST_i686-darwin-macos with the value 
# i386.
define DEF_HOST_VAR
  HOST_$(1) = $(subst i686,i386,$(word 1,$(subst -, ,$(1))))
endef
$(foreach t,$(CFG_TARGET_TRIPLES),$(eval $(call DEF_HOST_VAR,$(t))))
$(foreach t,$(CFG_TARGET_TRIPLES),$(info cfg: host for $(t) is $(HOST_$(t))))

# Ditto for OSTYPE
define DEF_OSTYPE_VAR
  OSTYPE_$(1) = $(subst $(firstword $(subst -, ,$(1)))-,,$(1))
endef
$(foreach t,$(CFG_TARGET_TRIPLES),$(eval $(call DEF_OSTYPE_VAR,$(t))))
$(foreach t,$(CFG_TARGET_TRIPLES),$(info cfg: os for $(t) is $(OSTYPE_$(t))))

# FIXME: no-omit-frame-pointer is just so that task_start_wrapper
# has a frame pointer and the stack walker can understand it. Turning off
# frame pointers everywhere is overkill
CFG_GCCISH_CFLAGS += -fno-omit-frame-pointer

# On Darwin, we need to run dsymutil so the debugging information ends
# up in the right place.  On other platforms, it automatically gets
# embedded into the executable, so use a no-op command.
CFG_DSYMUTIL := true

# Hack: not sure how to test if a file exists in make other than this
OS_SUPP = $(patsubst %,--suppressions=%,\
      $(wildcard $(CFG_SRC_DIR)src/etc/$(CFG_OSTYPE).supp*))

ifdef CFG_DISABLE_OPTIMIZE_CXX
  $(info cfg: disabling C++ optimization (CFG_DISABLE_OPTIMIZE_CXX))
  CFG_GCCISH_CFLAGS += -O0
else
  CFG_GCCISH_CFLAGS += -O2
endif

ifdef CFG_VALGRIND
  CFG_VALGRIND += --error-exitcode=100 \
                  --quiet \
                  --suppressions=$(CFG_SRC_DIR)src/etc/x86.supp \
                  $(OS_SUPP)
  ifdef CFG_ENABLE_HELGRIND
    CFG_VALGRIND += --tool=helgrind
  else
    CFG_VALGRIND += --tool=memcheck \
                    --leak-check=full
  endif
endif

ifneq ($(findstring linux,$(CFG_OSTYPE)),)
  # -znoexecstack is here because librt is for some reason being created
  # with executable stack and Fedora (or SELinux) doesn't like that (#798)
  ifdef CFG_PERF
    ifneq ($(CFG_PERF_WITH_LOGFD),)
        CFG_PERF_TOOL := $(CFG_PERF) stat -r 3 --log-fd 2
    else
        CFG_PERF_TOOL := $(CFG_PERF) stat -r 3
    endif
  else
    ifdef CFG_VALGRIND
      CFG_PERF_TOOL :=\
        $(CFG_VALGRIND) --tool=cachegrind --cache-sim=yes --branch-sim=yes
    else
      CFG_PERF_TOOL := /usr/bin/time --verbose
    endif
  endif
endif

# These flags will cause the compiler to produce a .d file
# next to the .o file that lists header deps.
CFG_DEPEND_FLAGS = -MMD -MP -MT $(1) -MF $(1:%.o=%.d)

AR := ar

CFG_INFO := $(info cfg: using $(CFG_C_COMPILER))
ifeq ($(CFG_C_COMPILER),clang)
  ifeq ($(origin CC),default)
    CC=clang
  endif
  ifeq ($(origin CXX),default)
    CXX=clang++
  endif
  ifeq ($(origin CPP),default)
    CPP=clang
  endif
else
ifeq ($(CFG_C_COMPILER),gcc)
  ifeq ($(origin CC),default)
    CC=gcc
  endif
  ifeq ($(origin CXX),default)
    CXX=g++
  endif
  ifeq ($(origin CPP),default)
    CPP=gcc
  endif
else
  CFG_ERR := $(error please try on a system with gcc or clang)
endif
endif


# x86_64-unknown-linux-gnu configuration
CC_x86_64-unknown-linux-gnu=$(CC)
CXX_x86_64-unknown-linux-gnu=$(CXX)
CPP_x86_64-unknown-linux-gnu=$(CPP)
AR_x86_64-unknown-linux-gnu=$(AR)
CFG_LIB_NAME_x86_64-unknown-linux-gnu=lib$(1).so
CFG_LIB_GLOB_x86_64-unknown-linux-gnu=lib$(1)-*.so
CFG_LIB_DSYM_GLOB_x86_64-unknown-linux-gnu=lib$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_x86_64-unknown-linux-gnu := -Wall -Werror -g -fPIC -m64
CFG_GCCISH_CXXFLAGS_x86_64-unknown-linux-gnu := -fno-rtti
CFG_GCCISH_LINK_FLAGS_x86_64-unknown-linux-gnu := -shared -fPIC -ldl -lpthread -lrt -g -m64
CFG_GCCISH_DEF_FLAG_x86_64-unknown-linux-gnu := -Wl,--export-dynamic,--dynamic-list=
CFG_GCCISH_PRE_LIB_FLAGS_x86_64-unknown-linux-gnu := -Wl,-whole-archive
CFG_GCCISH_POST_LIB_FLAGS_x86_64-unknown-linux-gnu := -Wl,-no-whole-archive -Wl,-znoexecstack
CFG_DEF_SUFFIX_x86_64-unknown-linux-gnu := .linux.def
CFG_INSTALL_NAME_x86_64-unknown-linux-gnu =
CFG_LIBUV_LINK_FLAGS_x86_64-unknown-linux-gnu =
CFG_LLVM_BUILD_ENV_x86_64-unknown-linux-gnu="CXXFLAGS=-fno-omit-frame-pointer"
CFG_EXE_SUFFIX_x86_64-unknown-linux-gnu =
CFG_WINDOWSY_x86_64-unknown-linux-gnu :=
CFG_UNIXY_x86_64-unknown-linux-gnu := 1
CFG_PATH_MUNGE_x86_64-unknown-linux-gnu := true
CFG_LDPATH_x86_64-unknown-linux-gnu :=
CFG_RUN_x86_64-unknown-linux-gnu=$(2)
CFG_RUN_TARG_x86_64-unknown-linux-gnu=$(call CFG_RUN_x86_64-unknown-linux-gnu,,$(2))

# i686-unknown-linux-gnu configuration
CC_i686-unknown-linux-gnu=$(CC)
CXX_i686-unknown-linux-gnu=$(CXX)
CPP_i686-unknown-linux-gnu=$(CPP)
AR_i686-unknown-linux-gnu=$(AR)
CFG_LIB_NAME_i686-unknown-linux-gnu=lib$(1).so
CFG_LIB_GLOB_i686-unknown-linux-gnu=lib$(1)-*.so
CFG_LIB_DSYM_GLOB_i686-unknown-linux-gnu=lib$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_i686-unknown-linux-gnu := -Wall -Werror -g -fPIC -m32
CFG_GCCISH_CXXFLAGS_i686-unknown-linux-gnu := -fno-rtti
CFG_GCCISH_LINK_FLAGS_i686-unknown-linux-gnu := -shared -fPIC -ldl -lpthread -lrt -g -m32
CFG_GCCISH_DEF_FLAG_i686-unknown-linux-gnu := -Wl,--export-dynamic,--dynamic-list=
CFG_GCCISH_PRE_LIB_FLAGS_i686-unknown-linux-gnu := -Wl,-whole-archive
CFG_GCCISH_POST_LIB_FLAGS_i686-unknown-linux-gnu := -Wl,-no-whole-archive -Wl,-znoexecstack
CFG_DEF_SUFFIX_i686-unknown-linux-gnu := .linux.def
CFG_INSTALL_NAME_i686-unknown-linux-gnu =
CFG_LIBUV_LINK_FLAGS_i686-unknown-linux-gnu =
CFG_LLVM_BUILD_ENV_i686-unknown-linux-gnu="CXXFLAGS=-fno-omit-frame-pointer"
CFG_EXE_SUFFIX_i686-unknown-linux-gnu =
CFG_WINDOWSY_i686-unknown-linux-gnu :=
CFG_UNIXY_i686-unknown-linux-gnu := 1
CFG_PATH_MUNGE_i686-unknown-linux-gnu := true
CFG_LDPATH_i686-unknown-linux-gnu :=
CFG_RUN_i686-unknown-linux-gnu=$(2)
CFG_RUN_TARG_i686-unknown-linux-gnu=$(call CFG_RUN_i686-unknown-linux-gnu,,$(2))

# x86_64-apple-darwin configuration
CC_x86_64-apple-darwin=$(CC)
CXX_x86_64-apple-darwin=$(CXX)
CPP_x86_64-apple-darwin=$(CPP)
AR_x86_64-apple-darwin=$(AR)
CFG_LIB_NAME_x86_64-apple-darwin=lib$(1).dylib
CFG_LIB_GLOB_x86_64-apple-darwin=lib$(1)-*.dylib
CFG_LIB_DSYM_GLOB_x86_64-apple-darwin=lib$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_x86_64-apple-darwin := -Wall -Werror -g -fPIC -m64 -arch x86_64
CFG_GCCISH_CXXFLAGS_x86_64-apple-darwin := -fno-rtti
CFG_GCCISH_LINK_FLAGS_x86_64-apple-darwin := -dynamiclib -lpthread -framework CoreServices -Wl,-no_compact_unwind -m64
CFG_GCCISH_DEF_FLAG_x86_64-apple-darwin := -Wl,-exported_symbols_list,
CFG_GCCISH_PRE_LIB_FLAGS_x86_64-apple-darwin :=
CFG_GCCISH_POST_LIB_FLAGS_x86_64-apple-darwin :=
CFG_DEF_SUFFIX_x86_64-apple-darwin := .darwin.def
CFG_INSTALL_NAME_x86_64-apple-darwin = -Wl,-install_name,@rpath/$(1)
CFG_LIBUV_LINK_FLAGS_x86_64-apple-darwin =
CFG_EXE_SUFFIX_x86_64-apple-darwin :=
CFG_WINDOWSY_x86_64-apple-darwin :=
CFG_UNIXY_x86_64-apple-darwin := 1
CFG_PATH_MUNGE_x86_64-apple-darwin := true
CFG_LDPATH_x86_64-apple-darwin :=
CFG_RUN_x86_64-apple-darwin=$(2)
CFG_RUN_TARG_x86_64-apple-darwin=$(call CFG_RUN_x86_64-apple-darwin,,$(2))

# i686-apple-darwin configuration
CC_i686-apple-darwin=$(CC)
CXX_i686-apple-darwin=$(CXX)
CPP_i686-apple-darwin=$(CPP)
AR_i686-apple-darwin=$(AR)
CFG_LIB_NAME_i686-apple-darwin=lib$(1).dylib
CFG_LIB_GLOB_i686-apple-darwin=lib$(1)-*.dylib
CFG_LIB_DSYM_GLOB_i686-apple-darwin=lib$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_i686-apple-darwin := -Wall -Werror -g -fPIC -m32 -arch i386
CFG_GCCISH_CXXFLAGS_i686-apple-darwin := -fno-rtti
CFG_GCCISH_LINK_FLAGS_i686-apple-darwin := -dynamiclib -lpthread -framework CoreServices -Wl,-no_compact_unwind -m32
CFG_GCCISH_DEF_FLAG_i686-apple-darwin := -Wl,-exported_symbols_list,
CFG_GCCISH_PRE_LIB_FLAGS_i686-apple-darwin :=
CFG_GCCISH_POST_LIB_FLAGS_i686-apple-darwin :=
CFG_DEF_SUFFIX_i686-apple-darwin := .darwin.def
CFG_INSTALL_NAME_i686-apple-darwin = -Wl,-install_name,@rpath/$(1)
CFG_LIBUV_LINK_FLAGS_i686-apple-darwin =
CFG_EXE_SUFFIX_i686-apple-darwin :=
CFG_WINDOWSY_i686-apple-darwin :=
CFG_UNIXY_i686-apple-darwin := 1
CFG_PATH_MUNGE_i686-apple-darwin := true
CFG_LDPATH_i686-apple-darwin :=
CFG_RUN_i686-apple-darwin=$(2)
CFG_RUN_TARG_i686-apple-darwin=$(call CFG_RUN_i686-apple-darwin,,$(2))

# arm-unknown-android configuration
CC_arm-unknown-android=$(CFG_ANDROID_CROSS_PATH)/bin/arm-linux-androideabi-gcc
CXX_arm-unknown-android=$(CFG_ANDROID_CROSS_PATH)/bin/arm-linux-androideabi-g++
CPP_arm-unknown-android=$(CFG_ANDROID_CROSS_PATH)/bin/arm-linux-androideabi-gcc -E
AR_arm-unknown-android=$(CFG_ANDROID_CROSS_PATH)/bin/arm-linux-androideabi-ar
CFG_LIB_NAME_arm-unknown-android=lib$(1).so
CFG_LIB_GLOB_arm-unknown-android=lib$(1)-*.so
CFG_LIB_DSYM_GLOB_arm-unknown-android=lib$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_arm-unknown-android := -Wall -g -fPIC -D__arm__ -DANDROID -D__ANDROID__
CFG_GCCISH_CXXFLAGS_arm-unknown-android := -fno-rtti
CFG_GCCISH_LINK_FLAGS_arm-unknown-android := -shared -fPIC -ldl -g -lm -lsupc++ -lgnustl_shared
CFG_GCCISH_DEF_FLAG_arm-unknown-android := -Wl,--export-dynamic,--dynamic-list=
CFG_GCCISH_PRE_LIB_FLAGS_arm-unknown-android := -Wl,-whole-archive
CFG_GCCISH_POST_LIB_FLAGS_arm-unknown-android := -Wl,-no-whole-archive -Wl,-znoexecstack
CFG_DEF_SUFFIX_arm-unknown-android := .android.def
CFG_INSTALL_NAME_arm-unknown-android =
CFG_LIBUV_LINK_FLAGS_arm-unknown-android =
CFG_EXE_SUFFIX_arm-unknown-android :=
CFG_WINDOWSY_arm-unknown-android :=
CFG_UNIXY_arm-unknown-android := 1
CFG_PATH_MUNGE_arm-unknown-android := true
CFG_LDPATH_arm-unknown-android :=
CFG_RUN_arm-unknown-android=
CFG_RUN_TARG_arm-unknown-android=
RUSTC_FLAGS_arm-unknown-android :=--android-cross-path='$(CFG_ANDROID_CROSS_PATH)'

# i686-pc-mingw32 configuration
CC_i686-pc-mingw32=$(CC)
CXX_i686-pc-mingw32=$(CXX)
CPP_i686-pc-mingw32=$(CPP)
AR_i686-pc-mingw32=$(AR)
CFG_LIB_NAME_i686-pc-mingw32=$(1).dll
CFG_LIB_GLOB_i686-pc-mingw32=$(1)-*.dll
CFG_LIB_DSYM_GLOB_i686-pc-mingw32=$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_i686-pc-mingw32 := -Wall -Werror -g -march=i686
CFG_GCCISH_CXXFLAGS_i686-pc-mingw32 := -fno-rtti
CFG_GCCISH_LINK_FLAGS_i686-pc-mingw32 := -shared -fPIC -g
CFG_GCCISH_DEF_FLAG_i686-pc-mingw32 :=
CFG_GCCISH_PRE_LIB_FLAGS_i686-pc-mingw32 := 
CFG_GCCISH_POST_LIB_FLAGS_i686-pc-mingw32 := 
CFG_DEF_SUFFIX_i686-pc-mingw32 := .mingw32.def
CFG_INSTALL_NAME_i686-pc-mingw32 =
CFG_LIBUV_LINK_FLAGS_i686-pc-mingw32 := -lWs2_32 -lpsapi -liphlpapi
CFG_EXE_SUFFIX_i686-pc-mingw32 := .exe
CFG_WINDOWSY_i686-pc-mingw32 := 1
CFG_UNIXY_i686-pc-mingw32 :=
CFG_PATH_MUNGE_i686-pc-mingw32 :=
CFG_LDPATH_i686-pc-mingw32 :=$(CFG_LDPATH_i686-pc-mingw32):$(PATH)
CFG_RUN_i686-pc-mingw32=PATH="$(CFG_LDPATH_i686-pc-mingw32):$(1)" $(2)
CFG_RUN_TARG_i686-pc-mingw32=$(call CFG_RUN_i686-pc-mingw32,$(HLIB$(1)_H_$(CFG_BUILD_TRIPLE)),$(2))

# i586-mingw32msvc configuration
CC_i586-mingw32msvc=$(CC)
CXX_i586-mingw32msvc=$(CXX)
CPP_i586-mingw32msvc=$(CPP)
AR_i586-mingw32msvc=$(AR)
CFG_LIB_NAME_i586-mingw32msvc=$(1).dll
CFG_LIB_GLOB_i586-mingw32msvc=$(1)-*.dll
CFG_LIB_DSYM_GLOB_i586-mingw32msvc=$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_i586-mingw32msvc := -Wall -Werror -g -march=586 -m32
CFG_GCCISH_CXXFLAGS_i586-mingw32msvc := -fno-rtti
CFG_GCCISH_LINK_FLAGS_i586-mingw32msvc := -shared -g -m32
CFG_GCCISH_DEF_FLAG_i586-mingw32msvc :=
CFG_GCCISH_PRE_LIB_FLAGS_i586-mingw32msvc :=
CFG_GCCISH_POST_LIB_FLAGS_i586-mingw32msvc :=
CFG_DEF_SUFFIX_i586-mingw32msvc := .mingw32.def
CFG_INSTALL_NAME_i586-mingw32msvc =
CFG_LIBUV_LINK_FLAGS_i586-mingw32msvc := -lWs2_32 -lpsapi -liphlpapi
CFG_EXE_SUFFIX_i586-mingw32msvc := .exe
CFG_WINDOWSY_i586-mingw32msvc := 1
CFG_UNIXY_i586-mingw32msvc :=
CFG_PATH_MUNGE_i586-mingw32msvc := $(strip perl -i.bak -p   \
                             -e 's@\\(\S)@/\1@go;'       \
                             -e 's@^/([a-zA-Z])/@\1:/@o;')
CFG_LDPATH_i586-mingw32msvc :=
CFG_RUN_i586-mingw32msvc=
CFG_RUN_TARG_i586-mingw32msvc=

# x86_64-unknown-freebsd configuration
CC_x86_64-unknown-freebsd=$(CC)
CXX_x86_64-unknown-freebsd=$(CXX)
CPP_x86_64-unknown-freebsd=$(CPP)
AR_x86_64-unknown-freebsd=$(AR)
CFG_LIB_NAME_x86_64-unknown-freebsd=lib$(1).so
CFG_LIB_GLOB_x86_64-unknown-freebsd=lib$(1)-*.so
CFG_LIB_DSYM_GLOB_x86_64-unknown-freebsd=$(1)-*.dylib.dSYM
CFG_GCCISH_CFLAGS_x86_64-unknown-freebsd := -Wall -Werror -g -fPIC -I/usr/local/include
CFG_GCCISH_LINK_FLAGS_x86_64-unknown-freebsd := -shared -fPIC -g -lpthread -lrt
CFG_GCCISH_DEF_FLAG_x86_64-unknown-freebsd := -Wl,--export-dynamic,--dynamic-list=
CFG_GCCISH_PRE_LIB_FLAGS_x86_64-unknown-freebsd := -Wl,-whole-archive
CFG_GCCISH_POST_LIB_FLAGS_x86_64-unknown-freebsd := -Wl,-no-whole-archive
CFG_DEF_SUFFIX_x86_64-unknown-freebsd := .bsd.def
CFG_INSTALL_NAME_x86_64-unknown-freebsd =
CFG_LIBUV_LINK_FLAGS_x86_64-unknown-freebsd := -lpthread -lkvm
CFG_EXE_SUFFIX_x86_64-unknown-freebsd :=
CFG_WINDOWSY_x86_64-unknown-freebsd :=
CFG_UNIXY_x86_64-unknown-freebsd := 1
CFG_PATH_MUNGE_x86_64-unknown-freebsd :=
CFG_LDPATH_x86_64-unknown-freebsd :=
CFG_RUN_x86_64-unknown-freebsd=$(2)
CFG_RUN_TARG_x86_64-unknown-freebsd=$(call CFG_RUN_x86_64-unknown-freebsd,,$(2))


define CFG_MAKE_TOOLCHAIN
  CFG_COMPILE_C_$(1) = $$(CC_$(1))  \
        $$(CFG_GCCISH_CFLAGS)      \
        $$(CFG_GCCISH_CFLAGS_$(1)) \
        $$(CFG_DEPEND_FLAGS)       \
        -c -o $$(1) $$(2)
  CFG_LINK_C_$(1) = $$(CC_$(1)) \
        $$(CFG_GCCISH_LINK_FLAGS) -o $$(1)          \
        $$(CFG_GCCISH_LINK_FLAGS_$(1)))             \
        $$(CFG_GCCISH_DEF_FLAG_$(1))$$(3) $$(2)     \
        $$(call CFG_INSTALL_NAME_$(1),$$(4))
  CFG_COMPILE_CXX_$(1) = $$(CXX_$(1)) \
        $$(CFG_GCCISH_CFLAGS)      \
        $$(CFG_GCCISH_CXXFLAGS)    \
        $$(CFG_GCCISH_CFLAGS_$(1)) \
        $$(CFG_GCCISH_CXXFLAGS_$(1))    \
        $$(CFG_DEPEND_FLAGS)       \
        -c -o $$(1) $$(2)
  CFG_LINK_CXX_$(1) = $$(CXX_$(1)) \
        $$(CFG_GCCISH_LINK_FLAGS) -o $$(1)             \
        $$(CFG_GCCISH_LINK_FLAGS_$(1))                 \
        $$(CFG_GCCISH_DEF_FLAG_$(1))$$(3) $$(2)        \
        $$(call CFG_INSTALL_NAME_$(1),$$(4))

  ifneq ($(1),arm-unknown-android)

  # We're using llvm-mc as our assembler because it supports
  # .cfi pseudo-ops on mac
  CFG_ASSEMBLE_$(1)=$$(CPP_$(1)) -E $$(CFG_DEPEND_FLAGS) $$(2) | \
                    $$(LLVM_MC_$$(CFG_BUILD_TRIPLE)) \
                    -assemble \
                    -filetype=obj \
                    -triple=$(1) \
                    -o=$$(1)
  else

  # For the Android cross, use the Android assembler
  # XXX: We should be able to use the LLVM assembler
  CFG_ASSEMBLE_$(1)=$$(CXX_$(1)) $$(CFG_DEPEND_FLAGS) $$(2) -c -o $$(1)

  endif

endef

$(foreach target,$(CFG_TARGET_TRIPLES),\
  $(eval $(call CFG_MAKE_TOOLCHAIN,$(target))))
