# use the official Bun image
# see all versions at https://hub.docker.com/r/oven/bun/tags
FROM oven/bun:1-alpine as base-alpine
WORKDIR /usr/src/app

FROM oven/bun:1 as base
WORKDIR /usr/src/app

# install dependencies into temp directory
# this will cache them and speed up future builds
FROM base AS install
RUN mkdir -p /temp/dev
COPY package.json bun.lock /temp/dev/
RUN cd /temp/dev && bun install --frozen-lockfile

# install with --production (exclude devDependencies)
#RUN mkdir -p /temp/prod
#COPY package.json bun.lock /temp/prod/
#RUN cd /temp/prod && bun install --frozen-lockfile --production

# copy node_modules from temp directory
# then copy all (non-ignored) project files into the image
FROM base AS prerelease
COPY --from=install /temp/dev/node_modules node_modules
COPY . .
RUN ls -la /usr/src/app

# [optional] tests & build
ENV NODE_ENV=production
RUN bun run build
RUN rm -rf node_modules

# copy production dependencies and source code into final image
FROM base-alpine AS release
COPY --from=prerelease --chown=bun:bun /usr/src/app/ .

# run the app
USER bun
EXPOSE 3000/tcp
ENTRYPOINT [ "bun", "run", ".output/server/index.mjs" ]
