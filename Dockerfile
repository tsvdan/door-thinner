FROM rust:1.63

ARG PORT=3000

WORKDIR /
RUN mkdir uploads

COPY src .
COPY Cargo* .

RUN apt-get update && \
    apt-get install ffmpeg -y

RUN cargo version
RUN cargo build --release

CMD [ "./target/release/door-thinner" ]

EXPOSE $PORT