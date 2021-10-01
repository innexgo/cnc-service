#!/bin/bash
./target/debug/auth-service \
  --port=8079 \
  --database-url=postgres://postgres:toor@localhost/auth \
  --site-external-url=http://localhost:3000 \
  --mail-service-url=http://localhost:8078
