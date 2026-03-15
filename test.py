import requests

# Configuration
URL = "http://127.0.0.1:3000/upload"
FILE_PATH = "/Users/timizuoebideri/Documents/WORKING/ED/major-ai-changes/frontend/class-prime/src-tauri/target/release/bundle/dmg/class-prime_0.1.8_aarch64.dmg"  # Replace with a real file

# Metadata for the release
data = {
    "app_name": "classprime",
    "version": "1.0.3",
    "target": "darwin",
    "arch": "arm64",
    "notes": "Testing the GitHub upload integration.",
    "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUMGpnWHJoZERFQ0d2dkUvbzYvaG9MYnU1ZVVpS3NnWlZ5c2M2MHBaenhoaGVjWDdVWTFvRFAyenQ2RU1KL0QzZEJkeFVLdW92U2pEV2xvaFRxbmVhZ0trYlA1NnhzMWc0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzY5ODkzOTIzCWZpbGU6Y2xhc3MtcHJpbWVfMC4xLjJfYWFyY2g2NC5kbWcKSTBPUHJSaXcxTUhoMzROVm1UVGk1TFZ5OU1WSGZ1ZVlXcitTZW5wUjdXaWdhdTlUSTVJMU5MdTErbWxsSVFwQ0FldkpPaER3d3JJU2NLZHdKYVg2RGc9PQo=",
}


def upload_test():
    try:
        with open(FILE_PATH, "rb") as f:
            # The 'file' key must match the match arm in your Rust code
            files = {"file": (FILE_PATH.split("/")[-1], f, "application/octet-stream")}

            print(f"🚀 Uploading {data['app_name']} v{data['version']} to {URL}...")
            import os

            file_size = os.path.getsize(FILE_PATH)
            print(f"📄 File size: {file_size} bytes")

            response = requests.post(URL, data=data, files=files)

            if response.status_code == 201:
                print("✅ Success!")
                print(f"🔗 GitHub Download URL: {response.json()}")
            elif response.status_code == 409:
                print("⚠️  Version already exists in the database.")
            else:
                print(f"❌ Failed with status {response.status_code}")
                print(f"Response: {response.text}")

    except FileNotFoundError:
        print(f"❌ Error: Could not find file at {FILE_PATH}")
    except Exception as e:
        print(f"❌ An error occurred: {e}")


def get_latest_test():
    url = "http://127.0.0.1:3000/latest/classprime/darwin/arm64"
    print(f"\n🚀 Checking latest version at {url}...")
    try:
        response = requests.get(url)
        if response.status_code == 200:
            print("✅ Success!")
            print(f"📦 Latest Version: {response.json()}")
        elif response.status_code == 204:
            print("ℹ️  No version found.")
        else:
            print(f"❌ Failed with status {response.status_code}")
    except Exception as e:
        print(f"❌ An error occurred: {e}")


def download_latest_test():
    url = "http://127.0.0.1:3000/download/latest/classprime/darwin/arm64"
    print(f"\n🚀 Checking download redirect at {url}...")
    try:
        # allow_redirects=False to see the 307/302 response
        response = requests.get(url, allow_redirects=False)
        if response.status_code in [301, 302, 307, 308]:
            print("✅ Success! Redirect found.")
            print(f"🔗 Redirect URL: {response.headers['Location']}")
        elif response.status_code == 404:
            print("ℹ️  No release found to download.")
        else:
            print(f"❌ Unexpected status {response.status_code}")
    except Exception as e:
        print(f"❌ An error occurred: {e}")


def root_test():
    url = "http://127.0.0.1:3000/"
    print(f"\n🚀 Checking root route at {url}...")
    try:
        response = requests.get(url)
        if response.status_code == 200 and response.text == "Updater Service Running":
            print("✅ Success! Root route works.")
        else:
            print(f"❌ Failed. Status: {response.status_code}, Body: {response.text}")
    except Exception as e:
        print(f"❌ An error occurred: {e}")


if __name__ == "__main__":
    root_test()
    upload_test()
    get_latest_test()
    download_latest_test()
