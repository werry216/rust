#!/usr/bin/env python
# -*- coding: utf-8 -*-

# Copyright 2017 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

import sys
import re
import json
import datetime
import collections
import textwrap
try:
    import urllib2
except ImportError:
    import urllib.request as urllib2

# List of people to ping when the status of a tool changed.
MAINTAINERS = {
    'miri': '@oli-obk @RalfJung @eddyb',
    'clippy-driver': '@Manishearth @llogiq @mcarton @oli-obk',
    'rls': '@nrc @Xanewok',
    'rustfmt': '@nrc',
    'book': '@carols10cents @steveklabnik',
    'nomicon': '@frewsxcv @Gankro',
    'reference': '@steveklabnik @Havvy @matthewjasper @alercah',
    'rust-by-example': '@steveklabnik @marioidival @projektir',
}


def read_current_status(current_commit, path):
    '''Reads build status of `current_commit` from content of `history/*.tsv`
    '''
    with open(path, 'rU') as f:
        for line in f:
            (commit, status) = line.split('\t', 1)
            if commit == current_commit:
                return json.loads(status)
    return {}

def issue(
    title,
    tool,
    maintainers,
    relevant_pr_number,
    relevant_pr_user,
):
    # Open an issue about the toolstate failure.
    gh_url = 'https://api.github.com/repos/rust-lang/rust/issues'
    assignees = [x.strip() for x in maintainers.split('@') if x != '']
    assignees.append(relevant_pr_user)
    response = urllib2.urlopen(urllib2.Request(
        gh_url,
        json.dumps({
            'body': '''\
            @{}: your PR ({}) broke {}

            If you have the time it would be great if you could open a PR against {} that
            fixes the fallout from your PR.
            '''.format(relevant_pr_user, relevant_pr_number, tool, tool),
            'title': title,
            'assignees': assignees,
            'labels': ['T-compiler', 'I-nominated'],
        }),
        {
            'Authorization': 'token ' + github_token,
            'Content-Type': 'application/json',
        }
    ))
    response.read()

def update_latest(
    current_commit,
    relevant_pr_number,
    relevant_pr_url,
    relevant_pr_user,
    current_datetime
):
    '''Updates `_data/latest.json` to match build result of the given commit.
    '''
    with open('_data/latest.json', 'rb+') as f:
        latest = json.load(f, object_pairs_hook=collections.OrderedDict)

        current_status = {
            os: read_current_status(current_commit, 'history/' + os + '.tsv')
            for os in ['windows', 'linux']
        }

        slug = 'rust-lang/rust'
        message = textwrap.dedent('''\
            📣 Toolstate changed by {}!

            Tested on commit {}@{}.
            Direct link to PR: <{}>

        ''').format(relevant_pr_number, slug, current_commit, relevant_pr_url)
        anything_changed = False
        for status in latest:
            tool = status['tool']
            changed = False

            for os, s in current_status.items():
                old = status[os]
                new = s.get(tool, old)
                status[os] = new
                if new > old:
                    changed = True
                    message += '🎉 {} on {}: {} → {} (cc {}, @rust-lang/infra).\n' \
                        .format(tool, os, old, new, MAINTAINERS.get(tool))
                elif new < old:
                    changed = True
                    title = '💔 {} on {}: {} → {}' \
                        .format(tool, os, old, new)
                    message += '{} (cc {}, @rust-lang/infra).\n' \
                        .format(title, MAINTAINERS.get(tool))
                    issue(title, tool, MAINTAINERS.get(tool), relevant_pr_number, relevant_pr_user)

            if changed:
                status['commit'] = current_commit
                status['datetime'] = current_datetime
                anything_changed = True

        if not anything_changed:
            return ''

        f.seek(0)
        f.truncate(0)
        json.dump(latest, f, indent=4, separators=(',', ': '))
        return message


if __name__ == '__main__':
    cur_commit = sys.argv[1]
    cur_datetime = datetime.datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ')
    cur_commit_msg = sys.argv[2]
    save_message_to_path = sys.argv[3]
    github_token = sys.argv[4]

    relevant_pr_match = re.search('Auto merge of #([0-9]+) - ([^:]+)', cur_commit_msg)
    if relevant_pr_match:
        number = relevant_pr_match.group(1)
        relevant_pr_user = relevant_pr_match.group(2)
        relevant_pr_number = 'rust-lang/rust#' + number
        relevant_pr_url = 'https://github.com/rust-lang/rust/pull/' + number
    else:
        number = '-1'
        relevant_pr_user = '<unknown user>'
        relevant_pr_number = '<unknown PR>'
        relevant_pr_url = '<unknown>'

    message = update_latest(
        cur_commit,
        relevant_pr_number,
        relevant_pr_url,
        relevant_pr_user,
        cur_datetime
    )
    if not message:
        print('<Nothing changed>')
        sys.exit(0)

    print(message)
    with open(save_message_to_path, 'w') as f:
        f.write(message)

    # Write the toolstate comment on the PR as well.
    gh_url = 'https://api.github.com/repos/rust-lang/rust/issues/{}/comments' \
        .format(number)
    response = urllib2.urlopen(urllib2.Request(
        gh_url,
        json.dumps({'body': message}),
        {
            'Authorization': 'token ' + github_token,
            'Content-Type': 'application/json',
        }
    ))
    response.read()
