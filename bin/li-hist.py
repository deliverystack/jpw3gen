import pandas as pd
import re
import os
import html

# --- CONFIGURATION ---
# 1. URL Shortening
URL_LENGTH_THRESHOLD = 200 

# 2. Fuzzy Matching for "Orphaned" Comments
ID_PREFIX_LENGTH = 8
# ---------------------

def shorten_urls(text, length_threshold=URL_LENGTH_THRESHOLD):
    """
    Finds URLs in text and converts them to Markdown links.
    """
    if not text:
        return ""
    
    url_pattern = re.compile(r'([\[\(]?)((?:https?://|www\.)[^\s\[\]\(\)]+)([\]\)]?)')
    
    def replace_match(match):
        opener = match.group(1)
        full_url = match.group(2)
        closer = match.group(3)
        
        # 1. Strip garbage suffixes
        if '|leo://' in full_url:
            full_url = full_url.split('|leo://')[0]

        # 2. Clean trailing punctuation
        while full_url and full_url[-1] in '.,;':
            full_url = full_url[:-1]

        # 3. Check Length
        if len(full_url) > length_threshold:
            display_text = "link shortened"
        else:
            display_text = full_url
            
        markdown_link = f'[{display_text}]({full_url})'
        
        # 4. Handle Surrounding Brackets
        if opener == '[' and closer == ']':
            return markdown_link
        elif opener == '(' and closer == ')':
            return f'({markdown_link})'
        else:
            return f'{opener}{markdown_link}{closer}'

    return url_pattern.sub(replace_match, text)

def clean_share_content(raw_text):
    """
    Cleans the messy quoting from the Shares CSV content.
    """
    if not raw_text:
        return ""
    
    # 1. Fix paragraph breaks
    text = raw_text.replace('""""', '\n\n')
    
    lines = text.split('\n')
    cleaned_lines = []
    
    for line in lines:
        line = line.strip()
        
        # --- IMPROVED QUOTE STRIPPING ---
        # Handle start and end independently to catch mismatched artifacts
        # (e.g. line starts with "" but ends with only ")
        
        # 1. Strip Leading Quotes
        if line.startswith('""'):
            line = line[2:]
        elif line.startswith('"'):
            line = line[1:]
            
        # 2. Strip Trailing Quotes
        if line.endswith('""'):
            line = line[:-2]
        elif line.endswith('"'):
            line = line[:-1]
        
        # 3. Fix internal escaped quotes (CSV uses "" for literal ")
        line = line.replace('""', '"')
        
        cleaned_lines.append(line)
    
    text = "\n".join(cleaned_lines)
    
    # HTML and Entity Cleaning
    text = re.sub(r'<br\s*/?>', '\n', text, flags=re.IGNORECASE)
    text = html.unescape(text)
    text = html.escape(text, quote=False)

    # Shorten & Linkify URLs
    text = shorten_urls(text)

    return text

def extract_id_prefix(url, length=ID_PREFIX_LENGTH):
    """
    Extracts the long numeric ID from a LinkedIn URL and returns the first N digits.
    """
    if not isinstance(url, str):
        return None
    match = re.search(r'(\d{15,})', url)
    if match:
        full_id = match.group(1)
        return full_id[:length]
    return None

def parse_shares(filename):
    """
    Parses Shares.csv line-by-line.
    """
    posts = []
    if not os.path.exists(filename):
        print(f"File not found: {filename}")
        return pd.DataFrame()

    with open(filename, 'r', encoding='utf-8', errors='replace') as f:
        lines = f.readlines()

    current_lines = []
    
    def process_buffer(buffer):
        if not buffer:
            return
        
        first_line = buffer[0]
        parts = first_line.split(',', 2)
        if len(parts) < 2: return 
        
        date = parts[0].strip()
        link = parts[1].strip()
        content_parts = []
        
        if len(parts) > 2:
            if len(buffer) == 1:
                content_part = parts[2].rsplit(',', 3)[0]
            else:
                content_part = parts[2]
            content_parts.append(content_part)
        
        for i in range(1, len(buffer) - 1):
            content_parts.append(buffer[i])
            
        if len(buffer) > 1:
            last_line = buffer[-1]
            content_part = last_line.rsplit(',', 3)[0]
            content_parts.append(content_part)
            
        full_raw_content = "\n".join(content_parts)
        clean_content = clean_share_content(full_raw_content)
        
        posts.append({'Date': date, 'Link': link, 'Content': clean_content})

    date_pattern = re.compile(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},')
    start_idx = 1
    for line in lines[start_idx:]:
        if date_pattern.match(line):
            process_buffer(current_lines)
            current_lines = [line]
        else:
            current_lines.append(line)
    process_buffer(current_lines)
    
    return pd.DataFrame(posts)

def main():
    print("--- Starting Processing ---")
    print(f"URL Shortening Threshold: {URL_LENGTH_THRESHOLD} chars")
    print(f"ID Prefix Match Length:   {ID_PREFIX_LENGTH} digits")
    
    # 1. Parse Shares
    print("Reading Shares.csv...")
    df_shares = parse_shares('Shares.csv')
    print(f"Parsed {len(df_shares)} posts from Shares.csv")

    # 2. Parse Comments
    print("Reading Comments.csv...")
    try:
        df_comments = pd.read_csv('Comments.csv', escapechar='\\')
        df_comments['Message'] = df_comments['Message'].astype(str).apply(clean_share_content)
    except Exception as e:
        print(f"Error reading Comments.csv: {e}")
        return

    # 3. Fuzzy Match
    print("Building ID prefix map...")
    share_prefix_map = {}
    for link in df_shares['Link']:
        prefix = extract_id_prefix(link)
        if prefix:
            share_prefix_map[prefix] = link

    print("Merging datasets...")
    reparented_count = 0
    valid_share_links = set(df_shares['Link'])
    
    def normalize_link(comment_link):
        nonlocal reparented_count
        if comment_link in valid_share_links:
            return comment_link
        
        prefix = extract_id_prefix(comment_link)
        if prefix and prefix in share_prefix_map:
            reparented_count += 1
            return share_prefix_map[prefix]
        return comment_link

    df_comments['Link'] = df_comments['Link'].apply(normalize_link)
    print(f"Re-parented {reparented_count} comments using ID prefix matching.")

    comment_links = set(df_comments['Link'])
    missing_links = comment_links - valid_share_links
    
    print(f"Found {len(missing_links)} posts referenced in comments but missing from shares.")

    missing_posts = []
    for link in missing_links:
        link_comments = df_comments[df_comments['Link'] == link]
        earliest_date = link_comments['Date'].min()
        missing_posts.append({
            'Date': earliest_date,
            'Link': link,
            'Content': "Post content not available."
        })
    
    if missing_posts:
        df_missing = pd.DataFrame(missing_posts)
        all_posts = pd.concat([df_shares, df_missing], ignore_index=True)
    else:
        all_posts = df_shares

    # 4. Sort and Save
    all_posts['DateDT'] = pd.to_datetime(all_posts['Date'], errors='coerce')
    all_posts = all_posts.sort_values('DateDT', ascending=True)

    print("Generating Markdown file...")
    output_file = 'LinkedIn_Export_Final.md'
    comments_by_link = df_comments.groupby('Link')
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for _, row in all_posts.iterrows():
            date_str = str(row['Date'])
            link = row['Link']
            content = row['Content']
            
            f.write(f"* [{date_str}]({link})\n")
            
            if content and str(content).lower() != 'nan':
                for line in content.split('\n'):
                    if line.strip():
                        f.write(f"    > {line}  \n")
            
            if link in comments_by_link.groups:
                post_comments = comments_by_link.get_group(link).sort_values('Date')
                f.write("    * **Comments:**\n")
                for _, c_row in post_comments.iterrows():
                    c_date = c_row['Date']
                    c_msg = c_row['Message']
                    c_msg_indented = c_msg.replace('\n', '  \n        ')
                    f.write(f"        * {c_date}: {c_msg_indented}\n")

    print(f"Success! Saved to {os.path.abspath(output_file)}")

if __name__ == "__main__":
    main()