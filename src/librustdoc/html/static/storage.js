// From rust:
/* global resourcesSuffix */

var currentTheme = document.getElementById("themeStyle");
var mainTheme = document.getElementById("mainThemeStyle");

var savedHref = [];

function hasClass(elem, className) {
    return elem && elem.classList && elem.classList.contains(className);
}

function addClass(elem, className) {
    if (!elem || !elem.classList) {
        return;
    }
    elem.classList.add(className);
}

function removeClass(elem, className) {
    if (!elem || !elem.classList) {
        return;
    }
    elem.classList.remove(className);
}

function onEach(arr, func, reversed) {
    if (arr && arr.length > 0 && func) {
        var length = arr.length;
        var i;
        if (reversed !== true) {
            for (i = 0; i < length; ++i) {
                if (func(arr[i]) === true) {
                    return true;
                }
            }
        } else {
            for (i = length - 1; i >= 0; --i) {
                if (func(arr[i]) === true) {
                    return true;
                }
            }
        }
    }
    return false;
}

function onEachLazy(lazyArray, func, reversed) {
    return onEach(
        Array.prototype.slice.call(lazyArray),
        func,
        reversed);
}

function hasOwnProperty(obj, property) {
    return Object.prototype.hasOwnProperty.call(obj, property);
}

function usableLocalStorage() {
    // Check if the browser supports localStorage at all:
    if (typeof Storage === "undefined") {
        return false;
    }
    // Check if we can access it; this access will fail if the browser
    // preferences deny access to localStorage, e.g., to prevent storage of
    // "cookies" (or cookie-likes, as is the case here).
    try {
        return window.localStorage !== null && window.localStorage !== undefined;
    } catch(err) {
        // Storage is supported, but browser preferences deny access to it.
        return false;
    }
}

function updateLocalStorage(name, value) {
    if (usableLocalStorage()) {
        localStorage[name] = value;
    } else {
        // No Web Storage support so we do nothing
    }
}

function getCurrentValue(name) {
    if (usableLocalStorage() && localStorage[name] !== undefined) {
        return localStorage[name];
    }
    return null;
}

function switchTheme(styleElem, mainStyleElem, newTheme, saveTheme) {
    var fullBasicCss = "rustdoc" + resourcesSuffix + ".css";
    var fullNewTheme = newTheme + resourcesSuffix + ".css";
    var newHref = mainStyleElem.href.replace(fullBasicCss, fullNewTheme);

    if (styleElem.href === newHref) {
        return;
    }

    var found = false;
    if (savedHref.length === 0) {
        onEachLazy(document.getElementsByTagName("link"), function(el) {
            savedHref.push(el.href);
        });
    }
    onEach(savedHref, function(el) {
        if (el === newHref) {
            found = true;
            return true;
        }
    });
    if (found === true) {
        styleElem.href = newHref;
        // If this new value comes from a system setting or from the previously saved theme, no
        // need to save it.
        if (saveTheme === true) {
            updateLocalStorage("rustdoc-theme", newTheme);
        }
    }
}

function useSystemTheme(value) {
    if (value === undefined) {
        value = true;
    }

    updateLocalStorage("rustdoc-use-system-theme", value);

    // update the toggle if we're on the settings page
    var toggle = document.getElementById("use-system-theme");
    if (toggle && toggle instanceof HTMLInputElement) {
        toggle.checked = value;
    }
}

var updateSystemTheme = (function() {
    if (!window.matchMedia) {
        // fallback to the CSS computed value
        return function() {
            let cssTheme = getComputedStyle(document.documentElement)
                .getPropertyValue('content');

            switchTheme(
                currentTheme,
                mainTheme,
                JSON.parse(cssTheme) || light,
                true
            );
        };
    }

    // only listen to (prefers-color-scheme: dark) because light is the default
    var mql = window.matchMedia("(prefers-color-scheme: dark)");

    function handlePreferenceChange(mql) {
        // maybe the user has disabled the setting in the meantime!
        if (getCurrentValue("rustdoc-use-system-theme") !== "false") {
            var lightTheme = getCurrentValue("rustdoc-preferred-light-theme") || "light";
            var darkTheme = getCurrentValue("rustdoc-preferred-dark-theme") || "dark";

            if (mql.matches) {
                // prefers a dark theme
                switchTheme(currentTheme, mainTheme, darkTheme, true);
            } else {
                // prefers a light theme, or has no preference
                switchTheme(currentTheme, mainTheme, lightTheme, true);
            }

            // note: we save the theme so that it doesn't suddenly change when
            // the user disables "use-system-theme" and reloads the page or
            // navigates to another page
        }
    }

    mql.addListener(handlePreferenceChange);

    return function() {
        handlePreferenceChange(mql);
    };
})();

if (getCurrentValue("rustdoc-use-system-theme") !== "false" && window.matchMedia) {
    // call the function to initialize the theme at least once!
    updateSystemTheme();
} else {
    switchTheme(currentTheme, mainTheme,
                getCurrentValue("rustdoc-theme") || "light",
                false);
}
