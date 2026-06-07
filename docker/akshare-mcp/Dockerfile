FROM python:3.12-slim
RUN pip install --no-cache-dir akshare-one-mcp
EXPOSE 8081
CMD ["akshare-one-mcp", "--streamable-http", "--host", "0.0.0.0", "--port", "8081"]
