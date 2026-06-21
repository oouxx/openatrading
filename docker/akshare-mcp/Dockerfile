FROM python:3.12-slim
RUN pip install --no-cache-dir akshare-mcp
ADD config.py /app/config.py
EXPOSE 8081
CMD ["python", "-m", "akshare-mcp", "--transport", "streamable-http", "--config", "/app/config.py","--host", "0.0.0.0", "--port", "8081"]
