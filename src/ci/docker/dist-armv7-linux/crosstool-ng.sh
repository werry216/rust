set -ex

# Mirrored from https://github.com/crosstool-ng/crosstool-ng/archive/crosstool-ng-1.24.0.tar.gz
url="https://rust-lang-ci-mirrors.s3-us-west-1.amazonaws.com/rustc/crosstool-ng-1.24.0.tar.gz"
curl -Lf $url | tar xzf -
cd crosstool-ng-crosstool-ng-1.24.0
./bootstrap
./configure --prefix=/usr/local
make -j$(nproc)
make install
cd ..
rm -rf crosstool-ng-crosstool-ng-1.24.0
