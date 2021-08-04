# --- Build stage
FROM ekidd/rust-musl-builder AS builder
ADD . ./
RUN sudo chown -R rust:rust /home/rust && \
    cargo build
	# add the --release option in `cargo build` for enabling 
	# compiler optimizations (it'll cause a much longer compile-time).

# --- Runtime stage
FROM openwhisk/dockerskeleton
ENV FLASK_PROXY_PORT 8080
RUN apk --no-cache add ca-certificates
COPY --from=builder \
	/home/rust/src/target/x86_64-unknown-linux-musl/debug/insert \
	/action/exec
	# If the --release option is enabled for cargo, make sure to change
	# debug to release in the source directory of the COPY instruction.
	
# --- CMD
CMD ["/bin/bash", "-c", "cd actionProxy && python -u actionproxy.py"]
