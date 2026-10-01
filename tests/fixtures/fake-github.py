#!/usr/bin/python3
"""GitHub API contract fixture: records requests locally, never uses the network."""
import json
import pathlib
import re
import sys
config = sys.stdin.read()
def field(name):
    return re.search(r'^' + re.escape(name) + r' = "([^"\n]+)"$', config, re.M).group(1)
method, url = field('request'), field('url')
state = pathlib.Path(__file__).parent
with (state/'calls.jsonl').open('a') as stream:
    stream.write(json.dumps({'method':method,'url':url})+'\n')
body = {}
if method != 'GET':
    body = json.loads(pathlib.Path(field('data-binary')[1:]).read_text() or '{}')
status = 200
response = {}
if method == 'GET' and url.endswith('/users/example-org'):
    response = {'login':'example-org','type':'Organization'}
elif method == 'POST' and url.endswith('/orgs/example-org/repos'):
    assert body['private'] is True and body['auto_init'] is True
    assert body['name'] == 'repo'
    status = 201; response = {'id':12345,'full_name':'example-org/repo','private':True}
elif method == 'GET' and '/git/ref/heads/' in url:
    response = {'object':{'sha':'a'*40}}
elif method == 'POST' and url.endswith('/git/blobs'):
    assert body['encoding'] == 'base64'
    status = 201;response={'sha':'b'*40}
elif method == 'POST' and url.endswith('/git/trees'):
    assert 'base_tree' not in body
    assert body['tree'][0]['path'] == 'README.md'
    status = 201;response={'sha':'c'*40}
elif method == 'POST' and url.endswith('/git/commits'):
    assert body['parents'] == ['a'*40]
    assert body['tree'] == 'c'*40
    status=201;response={'sha':'d'*40}
elif method == 'PATCH' and '/git/refs/heads/' in url:
    assert body['force'] is False
    response={'ref':'refs/heads/main','object':{'sha':'d'*40}}
elif method == 'GET' and url.endswith('/repos/example-org/repo'):
    response={'id':12345,'full_name':'example-org/repo','private':True}
elif method == 'DELETE' and url.endswith('/repos/example-org/repo'):
    status=204
else:
    status=404
pathlib.Path(field('output')).write_text('' if status==204 else json.dumps(response))
pathlib.Path(field('dump-header')).write_text('X-GitHub-Request-Id: fixture-request\n')
sys.stdout.write(str(status))
