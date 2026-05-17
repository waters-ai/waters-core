FROM ubuntu:24.04 AS build
RUN apt-get update && apt-get install -y redis-server ca-certificates
COPY target/release/waters-node /usr/local/bin/waters-node
COPY deploy/waters-node.env /etc/waters-node/env
EXPOSE 42069 42070 42071 42072 42169

FROM ubuntu:24.04
RUN apt-get update && apt-get install -y redis-server ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/bin/waters-node /usr/local/bin/waters-node
COPY --from=build /etc/waters-node/env /etc/waters-node/env
COPY deploy/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
