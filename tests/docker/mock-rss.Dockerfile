FROM python:3.13-alpine
COPY mock-rss.py /mock-rss.py
EXPOSE 8888
CMD ["python3", "/mock-rss.py", "8888"]
