FROM rust:1.63

ARG PORT=3000

WORKDIR /

RUN apt-get update && \
    apt-get install ffmpeg -y

RUN mkdir uploads

# cache deps
COPY Cargo* .
RUN mkdir src
RUN echo "fn main () { println!(\"Hello world\"); }" > src/main.rs
RUN cargo build --release
RUN rm src/main.rs

# build app
COPY ./src ./src
RUN rm ./target/release/deps/door_thinner*
RUN cargo build --release

CMD [ "./target/release/door-thinner" ]

EXPOSE $PORT