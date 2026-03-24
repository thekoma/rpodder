# Lightweight test runner for rpodder integration tests.
# Contains curl + bash + test scripts. Exit code 0 = all pass.
FROM alpine:3.21

RUN apk add --no-cache curl bash jq

COPY tests/ /tests/
RUN chmod +x /tests/*.sh

ENTRYPOINT ["/bin/bash", "/tests/entrypoint.sh"]
