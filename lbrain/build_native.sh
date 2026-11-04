#!/usr/bin/env bash
set -e

echo "[YAHWEH/Clojure] Preparing AOT compilation classes directory..."
mkdir -p classes target

# 1. lbrain.native gen-class AOT 바이트코드 컴파일
clj -M:build -e "(compile 'lbrain.native)"

# 2. GraalVM native-image 로 C 공유 라이브러리(libyahweh.so) 생성
echo "[YAHWEH/Clojure] Building native shared library (libyahweh.so)..."
native-image \
  -cp "$(clj -Spath):classes" \
  --shared \
  -H:Name=target/libyahweh \
  -H:+ReportExceptionStackTraces \
  --no-fallback \
  --enable-all-security-services \
  lbrain.native.YahwehNative

echo "[YAHWEH/Clojure] Build successful: target/libyahweh.so"