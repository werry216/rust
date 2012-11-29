# This file is generated by gyp; do not edit.

TOOLSET := target
TARGET := uv
DEFS_Debug := '-D_LARGEFILE_SOURCE' \
	'-D_FILE_OFFSET_BITS=64' \
	'-D_GNU_SOURCE' \
	'-DEIO_STACKSIZE=262144' \
	'-DHAVE_CONFIG_H' \
	'-DEV_CONFIG_H="config_android.h"' \
	'-DEIO_CONFIG_H="config_android.h"' \
	'-DDEBUG' \
	'-D_DEBUG' \
	'-DEV_VERIFY=2'

# Flags passed to all source files.
CFLAGS_Debug :=  \
	-Wall \
	-ansi \
	-fvisibility=hidden \
	-g \
	--std=gnu89 \
	-pedantic \
	-Wall \
	-Wextra \
	-Wno-unused-parameter \
	-g \
	-O0

# Flags passed to only C files.
CFLAGS_C_Debug := 

# Flags passed to only C++ files.
CFLAGS_CC_Debug := -fno-rtti \
	-fno-exceptions

INCS_Debug := -I$(srcdir)/src/libuv/include \
	-I$(srcdir)/src/libuv/include/uv-private \
	-I$(srcdir)/src/libuv/src \
	-I$(srcdir)/src/libuv/src/unix/ev \
	-I$(srcdir)/src/libuv/src/ares/config_android

DEFS_Release := '-D_LARGEFILE_SOURCE' \
	'-D_FILE_OFFSET_BITS=64' \
	'-D_GNU_SOURCE' \
	'-DEIO_STACKSIZE=262144' \
	'-DHAVE_CONFIG_H' \
	'-DEV_CONFIG_H="config_android.h"' \
	'-DEIO_CONFIG_H="config_android.h"' \
	'-DNDEBUG'

# Flags passed to all source files.
CFLAGS_Release :=  \
	-Wall \
	-ansi \
	-fvisibility=hidden \
	-g \
	--std=gnu89 \
	-pedantic \
	-Wall \
	-Wextra \
	-Wno-unused-parameter \
	-O3 \
	-fomit-frame-pointer \
	-fdata-sections \
	-ffunction-sections

# Flags passed to only C files.
CFLAGS_C_Release := 

# Flags passed to only C++ files.
CFLAGS_CC_Release := -fno-rtti \
	-fno-exceptions

INCS_Release := -I$(srcdir)/src/libuv/include \
	-I$(srcdir)/src/libuv/include/uv-private \
	-I$(srcdir)/src/libuv/src \
	-I$(srcdir)/src/libuv/src/unix/ev \
	-I$(srcdir)/src/libuv/src/ares/config_android

OBJS := $(obj).target/$(TARGET)/src/libuv/src/uv-common.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_cancel.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares__close_sockets.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_data.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_destroy.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_expand_name.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_expand_string.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_fds.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_free_hostent.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_free_string.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_gethostbyaddr.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_gethostbyname.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares__get_hostent.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_getnameinfo.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_getopt.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_getsock.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_init.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_library_init.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_llist.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_mkquery.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_nowarn.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_options.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_aaaa_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_a_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_mx_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_ns_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_ptr_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_srv_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_parse_txt_reply.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_process.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_query.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares__read_line.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_search.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_send.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_strcasecmp.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_strdup.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_strerror.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_timeout.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares__timeval.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_version.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/ares_writev.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/bitncmp.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/inet_net_pton.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/inet_ntop.o \
	$(obj).target/$(TARGET)/src/libuv/src/ares/windows_port.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/core.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/uv-eio.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/fs.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/udp.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/tcp.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/pipe.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/tty.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/stream.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/cares.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/dl.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/error.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/thread.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/process.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/eio/eio.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/ev/ev.o \
	$(obj).target/$(TARGET)/src/libuv/src/unix/linux.o

# Add to the list of files we specially track dependencies for.
all_deps += $(OBJS)

# CFLAGS et al overrides must be target-local.
# See "Target-specific Variable Values" in the GNU Make manual.
$(OBJS): TOOLSET := $(TOOLSET)
$(OBJS): GYP_CFLAGS := $(DEFS_$(BUILDTYPE)) $(INCS_$(BUILDTYPE))  $(CFLAGS_$(BUILDTYPE)) $(CFLAGS_C_$(BUILDTYPE))
$(OBJS): GYP_CXXFLAGS := $(DEFS_$(BUILDTYPE)) $(INCS_$(BUILDTYPE))  $(CFLAGS_$(BUILDTYPE)) $(CFLAGS_CC_$(BUILDTYPE))

# Suffix rules, putting all outputs into $(obj).

$(obj).$(TOOLSET)/$(TARGET)/%.o: $(srcdir)/%.c FORCE_DO_CMD
	@$(call do_cmd,cc,1)

# Try building from generated source, too.

$(obj).$(TOOLSET)/$(TARGET)/%.o: $(obj).$(TOOLSET)/%.c FORCE_DO_CMD
	@$(call do_cmd,cc,1)

$(obj).$(TOOLSET)/$(TARGET)/%.o: $(obj)/%.c FORCE_DO_CMD
	@$(call do_cmd,cc,1)

# End of this set of suffix rules
### Rules for final target.

LIBS := -lm

$(obj).target/src/libuv/libuv.a: GYP_LDFLAGS := $(LDFLAGS_$(BUILDTYPE))
$(obj).target/src/libuv/libuv.a: LIBS := $(LIBS)
$(obj).target/src/libuv/libuv.a: TOOLSET := $(TOOLSET)
$(obj).target/src/libuv/libuv.a: $(OBJS) FORCE_DO_CMD
	$(call do_cmd,alink)

all_deps += $(obj).target/src/libuv/libuv.a
# Add target alias
.PHONY: uv
uv: $(obj).target/src/libuv/libuv.a

# Add target alias to "all" target.
.PHONY: all
all: uv

