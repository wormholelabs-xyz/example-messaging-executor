# https://bun.sh/guides/ecosystem/docker
FROM oven/bun:1.2.2@sha256:e9382fda475d1ff0a939e925db3ca5a91b3b26cd71f23410dc5363262384bbc2 AS base
WORKDIR /usr/src/app

FROM base AS install
RUN apt update && apt install -y g++ make python3
RUN mkdir -p /temp/prod
COPY package.json bun.lock /temp/prod/
RUN cd /temp/prod && bun install --frozen-lockfile --production

FROM base AS release
COPY --from=install /temp/prod/node_modules node_modules
COPY package.json package.json
COPY app app

USER bun
EXPOSE 3000/tcp
ENTRYPOINT [ "bun", "run", "start" ]
