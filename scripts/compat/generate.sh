#!/bin/sh
# Regenerates tests/compat/expected.json from Raoh for Java.
# Needs Java 25 and Maven; the Java artifacts are the versions pom.xml names.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cp_file=$(mktemp)
trap 'rm -f "$cp_file"' EXIT
mvn -q -f "$here/pom.xml" dependency:build-classpath -Dmdep.outputFile="$cp_file"
java -cp "$(cat "$cp_file")" "$here/Generate.java" \
    "$root/tests/compat/cases.json" "$root/tests/compat/expected.json"
