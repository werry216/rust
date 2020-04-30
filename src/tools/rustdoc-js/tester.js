const fs = require('fs');
const path = require('path');

function getNextStep(content, pos, stop) {
    while (pos < content.length && content[pos] !== stop &&
           (content[pos] === ' ' || content[pos] === '\t' || content[pos] === '\n')) {
        pos += 1;
    }
    if (pos >= content.length) {
        return null;
    }
    if (content[pos] !== stop) {
        return pos * -1;
    }
    return pos;
}

// Stupid function extractor based on indent. Doesn't support block
// comments. If someone puts a ' or an " in a block comment this
// will blow up. Template strings are not tested and might also be
// broken.
function extractFunction(content, functionName) {
    var indent = 0;
    var splitter = "function " + functionName + "(";

    while (true) {
        var start = content.indexOf(splitter);
        if (start === -1) {
            break;
        }
        var pos = start;
        while (pos < content.length && content[pos] !== ')') {
            pos += 1;
        }
        if (pos >= content.length) {
            break;
        }
        pos = getNextStep(content, pos + 1, '{');
        if (pos === null) {
            break;
        } else if (pos < 0) {
            content = content.slice(-pos);
            continue;
        }
        while (pos < content.length) {
            // Eat single-line comments
            if (content[pos] === '/' && pos > 0 && content[pos-1] === '/') {
                do {
                    pos += 1;
                } while (pos < content.length && content[pos] !== '\n');

            // Eat quoted strings
            } else if (content[pos] === '"' || content[pos] === "'" || content[pos] === "`") {
                var stop = content[pos];
                var is_escaped = false;
                do {
                    if (content[pos] === '\\') {
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                } while (pos < content.length &&
                         (content[pos] !== stop || content[pos - 1] === '\\'));

            // Otherwise, check for indent
            } else if (content[pos] === '{') {
                indent += 1;
            } else if (content[pos] === '}') {
                indent -= 1;
                if (indent === 0) {
                    return content.slice(start, pos + 1);
                }
            }
            pos += 1;
        }
        content = content.slice(start + 1);
    }
    return null;
}

// Stupid function extractor for array.
function extractArrayVariable(content, arrayName) {
    var splitter = "var " + arrayName;
    while (true) {
        var start = content.indexOf(splitter);
        if (start === -1) {
            break;
        }
        var pos = getNextStep(content, start, '=');
        if (pos === null) {
            break;
        } else if (pos < 0) {
            content = content.slice(-pos);
            continue;
        }
        pos = getNextStep(content, pos, '[');
        if (pos === null) {
            break;
        } else if (pos < 0) {
            content = content.slice(-pos);
            continue;
        }
        while (pos < content.length) {
            if (content[pos] === '"' || content[pos] === "'") {
                var stop = content[pos];
                do {
                    if (content[pos] === '\\') {
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                } while (pos < content.length &&
                         (content[pos] !== stop || content[pos - 1] === '\\'));
            } else if (content[pos] === ']' &&
                       pos + 1 < content.length &&
                       content[pos + 1] === ';') {
                return content.slice(start, pos + 2);
            }
            pos += 1;
        }
        content = content.slice(start + 1);
    }
    return null;
}

// Stupid function extractor for variable.
function extractVariable(content, varName) {
    var splitter = "var " + varName;
    while (true) {
        var start = content.indexOf(splitter);
        if (start === -1) {
            break;
        }
        var pos = getNextStep(content, start, '=');
        if (pos === null) {
            break;
        } else if (pos < 0) {
            content = content.slice(-pos);
            continue;
        }
        while (pos < content.length) {
            if (content[pos] === '"' || content[pos] === "'") {
                var stop = content[pos];
                do {
                    if (content[pos] === '\\') {
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                } while (pos < content.length &&
                         (content[pos] !== stop || content[pos - 1] === '\\'));
            } else if (content[pos] === ';' || content[pos] === ',') {
                return content.slice(start, pos + 1);
            }
            pos += 1;
        }
        content = content.slice(start + 1);
    }
    return null;
}

function loadContent(content) {
    var Module = module.constructor;
    var m = new Module();
    m._compile(content, "tmp.js");
    m.exports.ignore_order = content.indexOf("\n// ignore-order\n") !== -1 ||
        content.startsWith("// ignore-order\n");
    m.exports.exact_check = content.indexOf("\n// exact-check\n") !== -1 ||
        content.startsWith("// exact-check\n");
    m.exports.should_fail = content.indexOf("\n// should-fail\n") !== -1 ||
        content.startsWith("// should-fail\n");
    return m.exports;
}

function readFile(filePath) {
    return fs.readFileSync(filePath, 'utf8');
}

function loadThings(thingsToLoad, kindOfLoad, funcToCall, fileContent) {
    var content = '';
    for (var i = 0; i < thingsToLoad.length; ++i) {
        var tmp = funcToCall(fileContent, thingsToLoad[i]);
        if (tmp === null) {
            console.log('unable to find ' + kindOfLoad + ' "' + thingsToLoad[i] + '"');
            process.exit(1);
        }
        content += tmp;
        content += 'exports.' + thingsToLoad[i] + ' = ' + thingsToLoad[i] + ';';
    }
    return content;
}

function lookForEntry(entry, data) {
    for (var i = 0; i < data.length; ++i) {
        var allGood = true;
        for (var key in entry) {
            if (!entry.hasOwnProperty(key)) {
                continue;
            }
            var value = data[i][key];
            // To make our life easier, if there is a "parent" type, we add it to the path.
            if (key === 'path' && data[i]['parent'] !== undefined) {
                if (value.length > 0) {
                    value += '::' + data[i]['parent']['name'];
                } else {
                    value = data[i]['parent']['name'];
                }
            }
            if (value !== entry[key]) {
                allGood = false;
                break;
            }
        }
        if (allGood === true) {
            return i;
        }
    }
    return null;
}

function loadMainJsAndIndex(mainJs, aliases, searchIndex, crate) {
    if (searchIndex[searchIndex.length - 1].length === 0) {
        searchIndex.pop();
    }
    searchIndex.pop();
    var fullSearchIndex = searchIndex.join("\n") + '\nexports.rawSearchIndex = searchIndex;';
    searchIndex = loadContent(fullSearchIndex);
    var finalJS = "";

    var arraysToLoad = ["itemTypes"];
    var variablesToLoad = ["MAX_LEV_DISTANCE", "MAX_RESULTS", "NO_TYPE_FILTER",
                           "GENERICS_DATA", "NAME", "INPUTS_DATA", "OUTPUT_DATA",
                           "TY_PRIMITIVE", "TY_KEYWORD",
                           "levenshtein_row2"];
    // execQuery first parameter is built in getQuery (which takes in the search input).
    // execQuery last parameter is built in buildIndex.
    // buildIndex requires the hashmap from search-index.
    var functionsToLoad = ["buildHrefAndPath", "pathSplitter", "levenshtein", "validateResult",
                           "handleAliases", "getQuery", "buildIndex", "execQuery", "execSearch"];

    finalJS += 'window = { "currentCrate": "' + crate + '" };\n';
    finalJS += 'var rootPath = "../";\n';
    finalJS += aliases;
    finalJS += loadThings(arraysToLoad, 'array', extractArrayVariable, mainJs);
    finalJS += loadThings(variablesToLoad, 'variable', extractVariable, mainJs);
    finalJS += loadThings(functionsToLoad, 'function', extractFunction, mainJs);

    var loaded = loadContent(finalJS);
    var index = loaded.buildIndex(searchIndex.rawSearchIndex);
    // We make it "global" so that the "loaded.execSearch" function will find it.
    rawSearchIndex = searchIndex.rawSearchIndex;

    return [loaded, index];
}

function runSearch(query, expected, index, loaded, loadedFile, queryName) {
    const filter_crate = loadedFile.FILTER_CRATE;
    const ignore_order = loadedFile.ignore_order;
    const exact_check = loadedFile.exact_check;

    var results = loaded.execSearch(loaded.getQuery(query), index, filter_crate);
    var error_text = [];

    for (var key in expected) {
        if (!expected.hasOwnProperty(key)) {
            continue;
        }
        if (!results.hasOwnProperty(key)) {
            error_text.push('==> Unknown key "' + key + '"');
            break;
        }
        var entry = expected[key];
        var prev_pos = -1;
        for (var i = 0; i < entry.length; ++i) {
            var entry_pos = lookForEntry(entry[i], results[key]);
            if (entry_pos === null) {
                error_text.push(queryName + "==> Result not found in '" + key + "': '" +
                                JSON.stringify(entry[i]) + "'");
            } else if (exact_check === true && prev_pos + 1 !== entry_pos) {
                error_text.push(queryName + "==> Exact check failed at position " + (prev_pos + 1) +
                                ": expected '" + JSON.stringify(entry[i]) + "' but found '" +
                                JSON.stringify(results[key][i]) + "'");
            } else if (ignore_order === false && entry_pos < prev_pos) {
                error_text.push(queryName + "==> '" + JSON.stringify(entry[i]) + "' was supposed " +
                                "to be before '" + JSON.stringify(results[key][entry_pos]) + "'");
            } else {
                prev_pos = entry_pos;
            }
        }
    }
    return error_text;
}

function checkResult(error_text, loadedFile, displaySuccess) {
    if (error_text.length === 0 && loadedFile.should_fail === true) {
        console.log("FAILED");
        console.log("==> Test was supposed to fail but all items were found...");
    } else if (error_text.length !== 0 && loadedFile.should_fail === false) {
        console.log("FAILED");
        console.log(error_text.join("\n"));
    } else {
        if (displaySuccess) {
            console.log("OK");
        }
        return 0;
    }
    return 1;
}

function runChecks(testFile, loaded, index) {
    var loadedFile = loadContent(
        readFile(testFile) + 'exports.QUERY = QUERY;exports.EXPECTED = EXPECTED;');

    const expected = loadedFile.EXPECTED;
    const query = loadedFile.QUERY;

    if (Array.isArray(query)) {
        if (!Array.isArray(expected)) {
            console.log("FAILED");
            console.log("==> If QUERY variable is an array, EXPECTED should be an array too");
            return 1;
        } else if (query.length !== expected.length) {
            console.log("FAILED");
            console.log("==> QUERY variable should have the same length as EXPECTED");
            return 1;
        }
        for (var i = 0; i < query.length; ++i) {
            var error_text = runSearch(query[i], expected[i], index, loaded, loadedFile,
                "[ query `" + query[i] + "`]");
            if (checkResult(error_text, loadedFile, false) !== 0) {
                return 1;
            }
        }
        console.log("OK");
        return 0;
    }
    var error_text = runSearch(query, expected, index, loaded, loadedFile, "");
    return checkResult(error_text, loadedFile, true);
}

function load_files(doc_folder, resource_suffix, crate) {
    var mainJs = readFile(path.join(doc_folder, "main" + resource_suffix + ".js"));
    var aliases = readFile(path.join(doc_folder, "aliases" + resource_suffix + ".js"));
    var searchIndex = readFile(
        path.join(doc_folder, "search-index" + resource_suffix + ".js")).split("\n");

    return loadMainJsAndIndex(mainJs, aliases, searchIndex, crate);
}

function showHelp() {
    console.log("rustdoc-js options:");
    console.log("  --doc-folder [PATH]        : location of the generated doc folder");
    console.log("  --help                     : show this message then quit");
    console.log("  --crate-name [STRING]      : crate name to be used");
    console.log("  --test-file [PATH]         : location of the JS test file");
    console.log("  --test-folder [PATH]       : location of the JS tests folder");
    console.log("  --resource-suffix [STRING] : suffix to refer to the correct files");
}

function parseOptions(args) {
    var opts = {
        "crate_name": "",
        "resource_suffix": "",
        "doc_folder": "",
        "test_folder": "",
        "test_file": "",
    };
    var correspondances = {
        "--resource-suffix": "resource_suffix",
        "--doc-folder": "doc_folder",
        "--test-folder": "test_folder",
        "--test-file": "test_file",
        "--crate-name": "crate_name",
    };

    for (var i = 0; i < args.length; ++i) {
        if (args[i] === "--resource-suffix"
            || args[i] === "--doc-folder"
            || args[i] === "--test-folder"
            || args[i] === "--test-file"
            || args[i] === "--crate-name") {
            i += 1;
            if (i >= args.length) {
                console.log("Missing argument after `" + args[i - 1] + "` option.");
                return null;
            }
            opts[correspondances[args[i - 1]]] = args[i];
        } else if (args[i] === "--help") {
            showHelp();
            process.exit(0);
        } else {
            console.log("Unknown option `" + args[i] + "`.");
            console.log("Use `--help` to see the list of options");
            return null;
        }
    }
    if (opts["doc_folder"].length < 1) {
        console.log("Missing `--doc-folder` option.");
    } else if (opts["crate_name"].length < 1) {
        console.log("Missing `--crate-name` option.");
    } else if (opts["test_folder"].length < 1 && opts["test_file"].length < 1) {
        console.log("At least one of `--test-folder` or `--test-file` option is required.");
    } else {
        return opts;
    }
    return null;
}

function checkFile(test_file, opts, loaded, index) {
    const test_name = path.basename(test_file, ".js");

    process.stdout.write('Checking "' + test_name + '" ... ');
    return runChecks(test_file, loaded, index);
}

function main(argv) {
    var opts = parseOptions(argv.slice(2));
    if (opts === null) {
        return 1;
    }

    var [loaded, index] = load_files(
        opts["doc_folder"],
        opts["resource_suffix"],
        opts["crate_name"]);
    var errors = 0;

    if (opts["test_file"].length !== 0) {
        errors += checkFile(opts["test_file"], opts, loaded, index);
    }
    if (opts["test_folder"].length !== 0) {
        fs.readdirSync(opts["test_folder"]).forEach(function(file) {
            if (!file.endsWith(".js")) {
                return;
            }
            errors += checkFile(path.join(opts["test_folder"], file), opts, loaded, index);
        });
    }
    return errors > 0 ? 1 : 0;
}

process.exit(main(process.argv));
