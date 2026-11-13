#!/usr/bin/env python3
"""Function-level porting inventory: upstream libavoid (C++) -> src/avoid (Rust).

Usage:
    tools/audit/inventory.py            # (re)write docs/PORTING_INVENTORY.md
    tools/audit/inventory.py --check    # validate only; exit 1 on problems
    tools/audit/inventory.py --dump     # print every upstream item (debugging)

What it does
------------
1. Enumerates every item of every upstream libavoid `.h`/`.cpp` (except
   tests/ and dllexport.h): classes/structs,
   methods (ctors, dtors, operators, inline and pure virtual ones), free
   functions (incl. `static` helpers), enums (with enumerators), constants,
   namespace-scope variables, typedefs and `#define`s.  It uses its own
   tokenizer (comments/strings stripped), a small preprocessor (`#if 0`,
   `#ifdef X`, ... evaluated with an empty define set except include guards;
   items in inactive regions are kept but marked) and a brace/scope tracking
   declaration parser.  Header declarations and .cpp definitions are merged
   per (qualified name, parameter types, const-ness).
2. Maps each item to the Rust port (`src/avoid/<stem>.rs`): primarily via
   `// upstream: <file> <name>` comments, secondarily by name
   (camelCase -> snake_case `fn`, same-named Rust type/const, operator ->
   std trait impl/derive).
3. Applies the manual classifications in `tools/audit/classification.txt`
   (`not-ported` categories and `MISSING` descriptions).

`--check` exits non-zero if (a) a Rust `upstream:` reference names an
upstream file/item that does not exist, (b) an upstream item is neither
mapped nor classified, (c) a classification entry matches nothing, or (d) a
classification contradicts an `upstream:` comment mapping.

Python 3 standard library only.
"""

import fnmatch
import os
import re
import sys
from collections import OrderedDict, defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def _find_upstream():
    # The vendor tree is untracked; worktrees share it with the main checkout.
    cand = [os.path.join(ROOT, "vendor", "adaptagrams", "cola", "libavoid")]
    d = ROOT
    for _ in range(6):
        d = os.path.dirname(d)
        cand.append(os.path.join(d, "vendor", "adaptagrams", "cola", "libavoid"))
    for c in cand:
        if os.path.isfile(os.path.join(c, "router.h")):
            return c
    sys.exit("inventory.py: cannot find vendor/adaptagrams/cola/libavoid")


UPSTREAM = _find_upstream()
RUST_DIR = os.path.join(ROOT, "src", "avoid")
CLASSIFICATION = os.path.join(ROOT, "tools", "audit", "classification.txt")
OUTPUT = os.path.join(ROOT, "docs", "PORTING_INVENTORY.md")
FIXTURES_OPS = os.path.join(ROOT, "tests", "fixtures_ops.rs")
EXCLUDED = {"dllexport.h"}

# ---------------------------------------------------------------------------
# Tokenizer + preprocessor
# ---------------------------------------------------------------------------


class Tok:
    __slots__ = ("kind", "text", "line", "cond")

    def __init__(self, kind, text, line, cond):
        self.kind = kind  # 'id', 'num', 'str', 'chr', 'p'
        self.text = text
        self.line = line
        self.cond = cond  # None (active) or a label like 'dead (#if 0)'

    def __repr__(self):
        return "%s@%d" % (self.text, self.line)


PUNCT3 = {"...", "<<=", ">>=", "->*"}
PUNCT2 = {"::", "->", "++", "--", "<=", ">=", "==", "!=", "&&", "||",
          "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", ".*"}
# '<<' and '>>' are deliberately NOT combined so that template argument
# brackets stay balanced; operator names are reassembled by the parser.


def eval_pp_expr(expr, defined):
    e = re.sub(r"defined\s*\(\s*(\w+)\s*\)",
               lambda m: "1" if m.group(1) in defined else "0", expr)
    e = re.sub(r"defined\s+(\w+)",
               lambda m: "1" if m.group(1) in defined else "0", e)
    e = re.sub(r"\b[A-Za-z_]\w*\b", "0", e)
    e = e.replace("&&", " and ").replace("||", " or ")
    e = re.sub(r"!(?!=)", " not ", e)
    try:
        return bool(eval(e, {"__builtins__": {}}, {}))
    except Exception:
        return False


class Frame:
    def __init__(self, active, label, taken):
        self.active = active  # this branch active (ignoring parents)
        self.label = label  # label used when this branch is inactive
        self.taken = taken  # some branch of this #if chain was active
        self.origin = label


def pp_label(kind, arg):
    arg = arg.strip()
    if kind == "if" and arg in ("0", "false"):
        return "dead (#if 0)"
    return "conditional (#%s %s)" % (kind, arg)


class Macro:
    def __init__(self, name, params, body, line, cond):
        self.name, self.params, self.body, self.line, self.cond = name, params, body, line, cond


TOKENIZE_WARNINGS = []


def tokenize(path):
    """Return (tokens, macros) for a C++ file."""
    with open(path, encoding="utf-8", errors="replace") as f:
        src = f.read()
    toks = []
    macros = []
    stack = []
    defined = set()
    directives_seen = []  # (kind, arg) of the first directives, for guard detection
    i, n, line = 0, len(src), 1
    at_line_start = True

    def cur_cond():
        for fr in stack:
            if not fr.active:
                return fr.label
        return None

    def close_branch(fr, dline):
        # An inactive branch whose braces/parens do not balance (e.g. an
        # `#if 0` block that only opens some scopes) would derail the scope
        # tracker: drop its tokens and report it.
        if fr.active:
            return
        seg = toks[fr.start:]
        bal = sum(1 if t.text in "({[" else -1 if t.text in ")}]" else 0 for t in seg)
        if bal != 0:
            TOKENIZE_WARNINGS.append("%s:%d-%d: unbalanced brackets in inactive region (%s); its tokens were "
                                     "ignored" % (os.path.basename(path), fr.bline, dline, fr.label))
            del toks[fr.start:]

    def handle_directive(text, dline):
        m = re.match(r"\s*#\s*(\w*)\s*(.*)", text, re.S)
        if not m:
            return
        kind, arg = m.group(1), m.group(2).strip()
        directives_seen.append((kind, arg))
        if kind in ("if", "ifdef", "ifndef"):
            if kind == "ifdef":
                act = arg.split()[0] in defined if arg else False
            elif kind == "ifndef":
                act = (arg.split()[0] not in defined) if arg else True
            else:
                act = eval_pp_expr(arg, defined)
            fr = Frame(act, pp_label(kind, arg), act)
            fr.origin = "#%s %s" % (kind, arg)
            fr.start, fr.bline = len(toks), dline
            stack.append(fr)
        elif kind == "elif":
            fr = stack[-1]
            close_branch(fr, dline)
            fr.start, fr.bline = len(toks), dline
            act = (not fr.taken) and eval_pp_expr(arg, defined)
            fr.active = act
            fr.taken = fr.taken or act
            fr.label = "conditional (#elif %s)" % arg
        elif kind == "else":
            fr = stack[-1]
            close_branch(fr, dline)
            fr.start, fr.bline = len(toks), dline
            fr.active = not fr.taken
            fr.taken = True
            if fr.origin.startswith("#if 0"):
                fr.label = "dead (#else of #if 0)"
            else:
                fr.label = "conditional (#else of %s)" % fr.origin
        elif kind == "endif":
            close_branch(stack[-1], dline)
            stack.pop()
        elif kind == "define":
            mm = re.match(r"(\w+)(\([^)]*\))?\s*(.*)", arg, re.S)
            if not mm:
                return
            name = mm.group(1)
            # include guard: '#ifndef X' immediately followed by '#define X'
            # as the first two directives of the file.
            if (len(directives_seen) == 2 and directives_seen[0][0] == "ifndef"
                    and directives_seen[0][1].split()[0] == name):
                defined.add(name)
                return
            params = mm.group(2)
            body = re.sub(r"\s+", " ", mm.group(3).replace("\\\n", " ")).strip()
            macros.append(Macro(name, params, body, dline, cur_cond()))

    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
            at_line_start = True
            continue
        if c in " \t\r\f\v":
            i += 1
            continue
        if c == "\\" and i + 1 < n and src[i + 1] == "\n":
            i += 2
            line += 1
            continue
        if c == "/" and src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if c == "/" and src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            line += src.count("\n", i, j)
            i = j
            continue
        if c == "#" and at_line_start:
            # gather directive text with continuations, dropping comments
            dline = line
            buf = []
            j = i
            while j < n:
                ch = src[j]
                if ch == "\\" and j + 1 < n and src[j + 1] == "\n":
                    buf.append("\\\n")
                    j += 2
                    line += 1
                    continue
                if ch == "\n":
                    break
                if src.startswith("//", j):
                    k = src.find("\n", j)
                    j = n if k < 0 else k
                    continue
                if src.startswith("/*", j):
                    k = src.find("*/", j + 2)
                    k = n if k < 0 else k + 2
                    line += src.count("\n", j, k)
                    buf.append(" ")
                    j = k
                    continue
                buf.append(ch)
                j += 1
            handle_directive("".join(buf), dline)
            i = j
            continue
        at_line_start = False
        cond = cur_cond()
        if c == '"' or (c in "LuU" and i + 1 < n and src[i + 1] == '"'):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                if src[j] == "\\":
                    j += 1
                if src[j] == "\n":
                    line += 1
                j += 1
            toks.append(Tok("str", src[i:j + 1], line, cond))
            i = j + 1
            continue
        if c == "'":
            j = i + 1
            while j < n and src[j] != "'":
                if src[j] == "\\":
                    j += 1
                j += 1
            toks.append(Tok("chr", src[i:j + 1], line, cond))
            i = j + 1
            continue
        if c.isalpha() or c == "_":
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            toks.append(Tok("id", src[i:j], line, cond))
            i = j
            continue
        if c.isdigit() or (c == "." and i + 1 < n and src[i + 1].isdigit()):
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] in "._'" or
                             (src[j] in "+-" and src[j - 1] in "eEpP")):
                j += 1
            toks.append(Tok("num", src[i:j], line, cond))
            i = j
            continue
        if src[i:i + 3] in PUNCT3:
            toks.append(Tok("p", src[i:i + 3], line, cond))
            i += 3
            continue
        if src[i:i + 2] in PUNCT2:
            toks.append(Tok("p", src[i:i + 2], line, cond))
            i += 2
            continue
        toks.append(Tok("p", c, line, cond))
        i += 1
    return toks, macros


# ---------------------------------------------------------------------------
# Declaration parser
# ---------------------------------------------------------------------------

BUILTIN_TYPES = {"int", "double", "float", "char", "bool", "void", "short",
                 "long", "unsigned", "signed", "wchar_t", "auto"}
QUALIFIERS = {"const", "volatile",
              "struct", "class", "enum", "typename"}
DECL_SPECIFIERS = {"static", "inline", "virtual", "explicit", "extern",
                   "constexpr", "mutable", "friend", "AVOID_EXPORT", "typename"}


class Item:
    """One upstream item (merged over declaration + definition)."""

    def __init__(self, kind, qual, stem, path, line, cond):
        self.kind = kind  # class, struct, union, enum, method, ctor, dtor,
        # operator, function, constant, variable, typedef, define, macro
        self.qual = qual  # qualified name without 'Avoid::'
        self.stem = stem
        self.params = None  # pretty param list (functions)
        self.sig = None  # loose key
        self.nparams = None
        self.method_const = False
        self.locs = []  # (file, line, what, cond)
        self.cond = cond
        self.has_body = False
        self.pure = False
        self.defaulted = False
        self.flags = set()  # static, inline, virtual, friend, template
        self.enumerators = []
        self.text = ""  # typedef / define text
        self.owner_class = None
        self.status = None
        self.category = None
        self.reason = ""
        self.rust = []  # [(method, 'file:line', detail)]
        self.notes = []
        self.overload_ambiguous = False

    @property
    def name(self):
        return self.qual.split("::")[-1]

    @property
    def file(self):
        return self.locs[0][0]

    @property
    def line(self):
        return self.locs[0][1]

    def display(self):
        if self.kind in ("method", "ctor", "dtor", "operator", "function"):
            s = "%s(%s)" % (self.qual, self.params)
            if self.method_const:
                s += " const"
            return s
        if self.kind == "macro":
            return "%s(%s)" % (self.qual, self.params or "")
        return self.qual


def join_tokens(toks):
    out = ""
    prev = None
    for t in toks:
        s = t.text
        if prev is not None:
            if s in ("::", ",", ")", "]", ">", "*", "&", "&&", "...") or prev in ("::", "(", "[", "<", "~"):
                if s in ("*", "&", "&&") and prev not in ("*", "&", "(", "<", "::"):
                    out += " "
                elif s == ">" and prev == ">":
                    out += " "
            elif s in ("(", "[", "<") and prev not in (",",):
                if prev in (",",):
                    out += " "
            else:
                out += " "
        if prev == "," and s not in (")",):
            if not out.endswith(" "):
                out += " "
        out += s
        prev = s
    return out


def split_top(toks, sep=","):
    parts, cur, depth, adepth = [], [], 0, 0
    for t in toks:
        if t.text in "([{":
            depth += 1
        elif t.text in ")]}":
            depth -= 1
        elif t.text == "<":
            adepth += 1
        elif t.text == ">" and adepth > 0:
            adepth -= 1
        if t.text == sep and depth == 0 and adepth == 0:
            parts.append(cur)
            cur = []
        else:
            cur.append(t)
    if cur:
        parts.append(cur)
    return parts


def param_type_tokens(ptoks):
    """Strip default value and parameter name from one parameter."""
    # default
    d, a = 0, 0
    for k, t in enumerate(ptoks):
        if t.text in "([{":
            d += 1
        elif t.text in ")]}":
            d -= 1
        elif t.text == "<":
            a += 1
        elif t.text == ">" and a:
            a -= 1
        elif t.text == "=" and d == 0 and a == 0:
            ptoks = ptoks[:k]
            break
    # arrays
    while ptoks and ptoks[-1].text == "]":
        k = len(ptoks) - 1
        while k >= 0 and ptoks[k].text != "[":
            k -= 1
        ptoks = ptoks[:k]
    if len(ptoks) >= 2 and ptoks[-1].kind == "id" and ptoks[-1].text not in BUILTIN_TYPES:
        prev = ptoks[-2]
        if prev.text in ("*", "&", "&&", ">") or (prev.kind == "id" and prev.text not in QUALIFIERS):
            ptoks = ptoks[:-1]
        elif prev.text == "const" and len(ptoks) >= 3 and ptoks[-3].text in ("*", "&"):
            ptoks = ptoks[:-1]  # `Variable *const v`
    return ptoks


def normalize_param_types(ptoks_list):
    pretty, loose = [], []
    for p in ptoks_list:
        tt = param_type_tokens(p)
        if not tt:
            continue
        pretty.append(join_tokens(tt))
        loose.append(loose_type([t.text for t in tt]))
    if pretty == ["void"]:
        pretty, loose = [], []
    return pretty, loose


def loose_type(words):
    w = [x for x in words if x not in ("const", "volatile", "struct", "class", "typename", "Avoid")]
    out = []
    for k, x in enumerate(w):
        if x == "::" and (k == 0 or w[k - 1] in ("Avoid",)):
            continue
        out.append(x)
    s = "".join(out)
    s = s.replace("Avoid::", "").replace("std::", "")
    s = re.sub(r"^unsigned(?!int|short|long|char)", "unsignedint", s)
    s = re.sub(r"unsigned(?=[*&]|$)", "unsignedint", s)
    if s == "void":
        return ""
    return s


class Parser:
    def __init__(self, path, stem, toks, items):
        self.path = path
        self.fname = os.path.basename(path)
        self.stem = stem
        self.t = toks
        self.n = len(toks)
        self.items = items  # list of raw items (dicts)
        self.warnings = []

    def txt(self, i):
        return self.t[i].text if i < self.n else ""

    def match(self, i):
        """index of the bracket matching self.t[i]."""
        o = self.t[i].text
        c = {"(": ")", "[": "]", "{": "}", "<": ">"}[o]
        d = 0
        j = i
        while j < self.n:
            x = self.t[j].text
            if o == "<":
                if x in ("(", "[", "{"):
                    j = self.match(j) + 1
                    continue
                if x in (";", "{", "}"):
                    return j - 1
            if x == o:
                d += 1
            elif x == c:
                d -= 1
                if d == 0:
                    return j
            j += 1
        return self.n - 1

    def add(self, **kw):
        kw.setdefault("file", self.fname)
        kw.setdefault("stem", self.stem)
        self.items.append(kw)

    def parse(self):
        i = self.parse_block(0, [], None)
        if i < self.n:
            self.warnings.append("%s: unbalanced '}' at line %d" % (self.fname, self.t[i].line))

    # scope: list of names (namespaces dropped if 'Avoid'); cls: enclosing class qual or None
    def parse_block(self, i, scope, cls):
        template_pending = False
        while i < self.n:
            t = self.t[i]
            x = t.text
            if x == "}":
                return i
            if x == ";":
                i += 1
                continue
            if x in ("public", "private", "protected") and self.txt(i + 1) == ":":
                i += 2
                continue
            if x == "namespace":
                j = i + 1
                name = None
                if self.t[j].kind == "id":
                    name = self.t[j].text
                    j += 1
                if self.txt(j) == "=":
                    i = self.skip_to_semi(j)
                    continue
                if self.txt(j) != "{":
                    i = j
                    continue
                sub = list(scope)
                if name and name != "Avoid":
                    sub.append(name)
                end = self.parse_block(j + 1, sub, cls)
                i = end + 1
                continue
            if x == "extern" and self.t[i + 1].kind == "str":
                if self.txt(i + 2) == "{":
                    end = self.parse_block(i + 3, scope, cls)
                    i = end + 1
                else:
                    i += 2
                continue
            if x == "template":
                if self.txt(i + 1) == "<":
                    i = self.match(i + 1) + 1
                else:
                    i += 1
                template_pending = True
                continue
            if x == "using":
                j = i + 1
                if self.t[j].kind == "id" and self.txt(j + 1) == "=":
                    e = self.skip_to_semi(j)
                    self.add(kind="typedef", qual=self.q(scope, cls, self.t[j].text), line=t.line,
                             cond=t.cond, text=join_tokens(self.t[i:e]), owner=cls)
                    i = e + 1
                else:
                    i = self.skip_to_semi(i) + 1
                continue
            if x == "typedef":
                i = self.parse_typedef(i, scope, cls)
                continue
            if x == "static_assert":
                i = self.skip_to_semi(i) + 1
                continue
            if x in ("class", "struct", "union"):
                r = self.try_class(i, scope, cls, template_pending)
                if r is not None:
                    i = r
                    template_pending = False
                    continue
            if x == "enum":
                r = self.try_enum(i, scope, cls)
                if r is not None:
                    i = r
                    continue
            if x == "friend":
                j = i + 1
                if self.txt(j) in ("class", "struct") and self.txt(j + 2) == ";":
                    i = j + 3
                    continue
                i = self.parse_decl(i, scope, cls, template_pending, friend=True)
                template_pending = False
                continue
            i = self.parse_decl(i, scope, cls, template_pending)
            template_pending = False
        return i

    def q(self, scope, cls, name):
        if cls:
            return cls + "::" + name
        return "::".join(list(scope) + [name])

    def skip_to_semi(self, i):
        while i < self.n:
            x = self.t[i].text
            if x in ("(", "[", "{"):
                i = self.match(i) + 1
                continue
            if x == ";":
                return i
            if x == "}":
                return i - 1  # let caller see the brace
            i += 1
        return i

    def try_class(self, i, scope, cls, template_pending):
        kw = self.t[i].text
        j = i + 1
        names = []
        while j < self.n and self.t[j].kind == "id":
            names.append(self.t[j])
            j += 1
        if self.txt(j) == "<":  # template specialisation
            j = self.match(j) + 1
        names = [nm for nm in names if nm.text not in ("final", "AVOID_EXPORT")]
        nx = self.txt(j)
        if nx == ";" and names and len(names) == 1:
            return j + 1  # forward declaration
        if nx not in ("{", ":"):
            return None
        if self.txt(j) == ":" and self.txt(j + 1) == ":":
            return None
        # find body
        k = j
        while k < self.n and self.t[k].text not in ("{", ";"):
            k += 1
        if self.txt(k) != "{":
            return None
        name = names[-1].text if names else "(anonymous %s)" % kw
        qual = self.q(scope, cls, name)
        self.add(kind=kw, qual=qual, line=self.t[i].line, cond=self.t[i].cond, owner=cls,
                 has_body=True, template=template_pending,
                 text=join_tokens(self.t[i:k]))
        end = self.parse_block(k + 1, scope, qual)
        # trailing declarators until ';'
        e = end + 1
        while e < self.n and self.t[e].text != ";":
            e += 1
        return e + 1

    def try_enum(self, i, scope, cls, typedef_name=None):
        j = i + 1
        if self.txt(j) in ("class", "struct"):
            j += 1
        name = None
        if self.t[j].kind == "id":
            name = self.t[j].text
            j += 1
        if self.txt(j) == ":":
            while self.txt(j) != "{" and self.txt(j) != ";":
                j += 1
        if self.txt(j) != "{":
            return None
        end = self.match(j)
        enumerators = []
        for part in split_top(self.t[j + 1:end]):
            if part and part[0].kind == "id":
                enumerators.append(part[0].text)
        if typedef_name and not name:
            name = typedef_name
        anon = name is None
        if anon:
            name = "enum {%s}" % (enumerators[0] + (", ..." if len(enumerators) > 1 else "")) if enumerators else "enum {}"
        self.add(kind="enum", qual=self.q(scope, cls, name), line=self.t[i].line, cond=self.t[i].cond,
                 owner=cls, has_body=True, enumerators=enumerators, anon=anon,
                 enum_lines=None)
        e = end + 1
        while e < self.n and self.t[e].text != ";":
            e += 1
        return e + 1

    def parse_typedef(self, i, scope, cls):
        t0 = self.t[i]
        j = i + 1
        # typedef enum/struct { ... } Name;
        if self.txt(j) in ("enum", "struct", "class", "union"):
            k = j + 1
            while k < self.n and self.t[k].kind == "id":
                k += 1
            if self.txt(k) == "{":
                end = self.match(k)
                e = end + 1
                while e < self.n and self.t[e].text != ";":
                    e += 1
                tdname = self.t[e - 1].text
                if self.txt(j) == "enum":
                    self.try_enum(j, scope, cls, typedef_name=tdname)
                    # if the enum was named, also record the typedef alias
                    if self.t[j + 1].kind == "id" and self.t[j + 1].text != tdname:
                        self.add(kind="typedef", qual=self.q(scope, cls, tdname), line=t0.line, cond=t0.cond,
                                 owner=cls, text="typedef enum %s %s" % (self.t[j + 1].text, tdname))
                else:
                    self.add(kind=self.txt(j), qual=self.q(scope, cls, tdname), line=t0.line,
                             cond=t0.cond, owner=cls, has_body=True, text="typedef struct {...} " + tdname)
                    self.parse_block(k + 1, scope, self.q(scope, cls, tdname))
                return e + 1
        e = self.skip_to_semi(i)
        body = self.t[i + 1:e]
        name = None
        # function pointer: ( * Name ) ( ... )
        for k in range(len(body) - 2):
            if body[k].text == "(" and body[k + 1].text in ("*", "&") and body[k + 2].kind == "id":
                name = body[k + 2].text
                break
        if name is None:
            b = list(body)
            while b and b[-1].text == "]":
                k = len(b) - 1
                while k >= 0 and b[k].text != "[":
                    k -= 1
                b = b[:k]
            for parts in split_top(b):
                pass
            name = b[-1].text if b else "?"
        self.add(kind="typedef", qual=self.q(scope, cls, name), line=t0.line, cond=t0.cond,
                 owner=cls, text=join_tokens(self.t[i:e]))
        return e + 1

    def parse_decl(self, i, scope, cls, template_pending, friend=False):
        start = i
        j = i
        while j < self.n:
            x = self.t[j].text
            if x == "}":
                return j
            if x == ";":
                self.record_var(start, j, scope, cls)
                return j + 1
            if x == "=" and self.txt(j - 1) == "operator":
                j += 1
                continue
            if x == "=":
                e = self.skip_to_semi(j)
                self.record_var(start, e, scope, cls)
                return e + 1
            if x == "{":
                # brace-initialised variable
                e = self.skip_to_semi(j)
                self.record_var(start, e, scope, cls)
                return e + 1
            if x == "[":
                j = self.match(j) + 1
                continue
            if x == "<" and j > start and self.t[j - 1].kind == "id" and self.t[j - 1].text != "operator":
                j = self.match(j) + 1
                continue
            if x == "(":
                # operator() ?
                if self.txt(j - 1) == "operator" and self.txt(j + 1) == ")":
                    j += 2
                    continue
                nxt = self.txt(j + 1)
                if nxt in ("*", "&") or (self.t[j + 1].kind == "id" and self.txt(j + 2) == "::" and self.txt(j + 3) == "*"):
                    # function pointer / pointer-to-member declarator
                    j = self.match(j) + 1
                    continue
                return self.record_func(start, j, scope, cls, template_pending, friend)
            j += 1
        return j

    def head_name(self, start, paren):
        """Return (qualifier list, name, name_start_index) for tokens before paren."""
        # operator?
        opk = None
        for k in range(start, paren):
            if self.t[k].text == "operator":
                opk = k
                break
        if opk is not None:
            optoks = [self.t[k].text for k in range(opk + 1, paren)]
            if all(len(s) <= 3 and not s[0].isalnum() for s in optoks):
                name = "operator" + "".join(optoks)
            else:
                name = "operator " + " ".join(optoks)
            k = opk
        else:
            k = paren - 1
            if self.t[k].kind != "id":
                return None, None, None
            name = self.t[k].text
            if self.txt(k - 1) == "~":
                name = "~" + name
                k -= 1
        quals = []
        while k - 2 >= start and self.t[k - 1].text == "::":
            p = k - 2
            if self.t[p].text == ">":
                d = 0
                while p >= start:
                    if self.t[p].text == ">":
                        d += 1
                    elif self.t[p].text == "<":
                        d -= 1
                        if d == 0:
                            break
                    p -= 1
                p -= 1
            if p < start or self.t[p].kind != "id":
                break
            quals.insert(0, self.t[p].text)
            k = p
        return quals, name, k

    def record_func(self, start, paren, scope, cls, template_pending, friend):
        quals, name, nstart = self.head_name(start, paren)
        pend = self.match(paren)
        if name is None or self.t[paren - 1].text in ("if", "while", "for", "switch", "return", "sizeof", "decltype"):
            # not a function declarator (e.g. macro invocation); skip statement
            e = pend + 1
            if self.txt(e) == "{":
                return self.match(e) + 1
            return self.skip_to_semi(e) + 1
        ptoks = self.t[paren + 1:pend]
        parts = split_top(ptoks)
        # variable with constructor arguments, e.g. `const VertID dummyOrthogID(0, 0);`
        is_var = any(p and (p[0].kind in ("num", "str", "chr") or p[0].text in ("-", "true", "false", "nullptr", "NULL"))
                     for p in parts)
        head = [self.t[k].text for k in range(start, nstart)]
        if is_var:
            e = self.skip_to_semi(pend)
            self.record_var(start, e, scope, cls)
            return e + 1
        j = pend + 1
        method_const = False
        pure = defaulted = False
        while j < self.n:
            x = self.t[j].text
            if x == "const":
                method_const = True
                j += 1
            elif x in ("volatile", "override", "final", "noexcept", "&", "&&"):
                if x == "noexcept" and self.txt(j + 1) == "(":
                    j = self.match(j + 1)
                j += 1
            elif x == "throw" and self.txt(j + 1) == "(":
                j = self.match(j + 1) + 1
            elif x == "->":
                j += 1
                while self.txt(j) not in ("{", ";", "="):
                    j += 1
            elif x == "=":
                v = self.txt(j + 1)
                if v == "0":
                    pure = True
                elif v in ("default", "delete"):
                    defaulted = v
                j += 2
            else:
                break
        has_body = False
        x = self.txt(j)
        if x == ":":
            # constructor initialiser list
            j += 1
            while j < self.n:
                y = self.t[j].text
                if y == "(" or y == "{":
                    j = self.match(j) + 1
                    if self.txt(j) == ",":
                        j += 1
                        continue
                    if self.txt(j) == "{":
                        break
                    continue
                if y == "<":
                    j = self.match(j) + 1
                    continue
                j += 1
            x = self.txt(j)
        if x == "{":
            has_body = True
            end = self.match(j)
            nxt = end + 1
        elif x == ";":
            nxt = j + 1
        elif x == ",":
            # several declarators; give up on the rest of the statement
            nxt = self.skip_to_semi(j) + 1
        else:
            self.warnings.append("%s:%d: unexpected token %r after declarator of %s" % (
                self.fname, self.t[j].line if j < self.n else -1, x, name))
            nxt = self.skip_to_semi(j) + 1
        pretty, loose = normalize_param_types(parts)
        # qualified name
        if friend:
            base = list(scope)
            qual = "::".join(base + quals + [name])
            owner = None
        elif cls:
            qual = cls + "::" + "::".join(quals + [name]) if quals else cls + "::" + name
            owner = cls
        else:
            qual = "::".join(list(scope) + quals + [name])
            owner = "::".join(list(scope) + quals) if quals else None
        cname = qual.split("::")
        if name.startswith("operator"):
            kind = "operator"
        elif name.startswith("~"):
            kind = "dtor"
        elif owner and len(cname) >= 2 and cname[-1] == cname[-2]:
            kind = "ctor"
        elif owner:
            kind = "method"
        else:
            kind = "function"
        flags = set(h for h in head if h in ("static", "inline", "virtual", "explicit"))
        if template_pending:
            flags.add("template")
        if friend:
            flags.add("friend")
        if has_body and cls:
            flags.add("in-class body")
        self.add(kind=kind, qual=qual, line=self.t[nstart].line, cond=self.t[nstart].cond,
                 owner=owner, has_body=has_body, pure=pure, defaulted=defaulted,
                 params=", ".join(pretty), loose=tuple(loose), method_const=method_const,
                 flags=flags, ret=join_tokens(self.t[start:nstart]))
        return nxt

    def record_var(self, start, end, scope, cls):
        toks = self.t[start:end]
        if not toks:
            return
        words = [t.text for t in toks]
        if words[0] in ("friend", "return", "using"):
            return
        # declarators
        decls = split_top(toks)
        if not decls:
            return
        first = decls[0]
        # find head (type) = tokens of the first declarator before its name
        def decl_name(dt):
            d = 0
            name_tok = None
            for k, t in enumerate(dt):
                if t.text in ("=", "{") and d == 0:
                    break
                if t.text in ("(", "[") and d == 0:
                    if name_tok is not None and t.text == "(":
                        break
                    if t.text == "[":
                        break
                if t.text in "([{":
                    d += 1
                elif t.text in ")]}":
                    d -= 1
                elif t.kind == "id" and d == 0:
                    name_tok = (k, t)
            return name_tok

        nt = decl_name(first)
        if nt is None:
            return
        k0, ntok = nt
        head = [t.text for t in first[:k0]]
        # qualified name in definitions like `const int Foo::BAR = 1;`
        quals = []
        k = k0
        while k - 2 >= 0 and first[k - 1].text == "::" and first[k - 2].kind == "id":
            quals.insert(0, first[k - 2].text)
            k -= 2
        type_head = [t.text for t in first[:k]]
        if not type_head or ntok.text in BUILTIN_TYPES:
            return  # not a declaration (e.g. stray macro use)
        is_const = "const" in type_head and not any(x in ("*",) for x in type_head[type_head.index("const"):]) \
            or "constexpr" in type_head
        is_static = "static" in type_head
        if cls and not is_static:
            return  # ordinary data member
        if "extern" in type_head and not is_const:
            pass
        kind = "constant" if is_const else "variable"
        names = [ntok.text]
        for d in decls[1:]:
            nt2 = decl_name(d)
            if nt2:
                names.append(nt2[1].text)
        for nm in names:
            if cls:
                qual = cls + "::" + nm
            else:
                qual = "::".join(list(scope) + quals + [nm])
            self.add(kind=kind, qual=qual, line=ntok.line, cond=ntok.cond, owner=cls,
                     has_body=("=" in words or "{" in words or "(" in words) and "extern" not in type_head,
                     text=re.sub(r"\s+", " ", join_tokens(toks))[:120],
                     flags=set(x for x in type_head if x in ("static", "extern")))


def parse_upstream():
    raw = []
    warnings = []
    files = sorted(f for f in os.listdir(UPSTREAM)
                   if (f.endswith(".h") or f.endswith(".cpp")) and f not in EXCLUDED)
    for f in files:
        path = os.path.join(UPSTREAM, f)
        stem = f.rsplit(".", 1)[0]
        toks, macros = tokenize(path)
        p = Parser(path, stem, toks, raw)
        p.parse()
        warnings += p.warnings
        for m in macros:
            raw.append(dict(kind="macro" if m.params is not None else "define", qual=m.name, file=f,
                            stem=stem, line=m.line, cond=m.cond, owner=None, has_body=True,
                            params=(m.params or "")[1:-1] if m.params else None,
                            text=("#define %s%s %s" % (m.name, m.params or "", m.body)).strip()))
    return files, raw, warnings + TOKENIZE_WARNINGS


def merge_items(raw):
    """Merge declarations and definitions into Items."""
    items = []
    by_key = {}
    fn_kinds = ("method", "ctor", "dtor", "operator", "function")
    pending_decls = []
    for r in raw:
        kind = r["kind"]
        if kind in fn_kinds:
            key = ("fn", r["qual"], r.get("loose"), r.get("method_const"))
        elif kind == "macro":
            key = ("macro", r["file"], r["qual"], r.get("params"))
        elif kind in ("constant", "variable"):
            key = ("var", r["qual"])
        elif kind in ("class", "struct", "union"):
            key = ("class", r["qual"])
        else:
            key = (kind, r["stem"], r["qual"])
        it = by_key.get(key)
        what = "def" if r.get("has_body") else "decl"
        if it is None:
            it = Item(kind, r["qual"], r["stem"], r["file"], r["line"], r.get("cond"))
            it.params = r.get("params")
            it.sig = r.get("loose")
            it.method_const = r.get("method_const", False)
            it.enumerators = r.get("enumerators", [])
            it.text = r.get("text", "")
            it.owner_class = r.get("owner")
            it.anon = r.get("anon", False)
            by_key[key] = it
            items.append(it)
        else:
            if r.get("cond") is None:
                it.cond = None
            if it.kind == "macro" and r.get("text") and r.get("text") not in it.text:
                it.text += " || " + r.get("text")
        it.locs.append((r["file"], r["line"], what, r.get("cond")))
        if r.get("has_body"):
            it.has_body = True
        if r.get("pure"):
            it.pure = True
        if r.get("defaulted"):
            it.defaulted = r["defaulted"]
        it.flags |= set(r.get("flags") or ())
    # Second pass: match declarations without body to unmatched same-name
    # definitions with the same arity (decl/def spelled differently).
    fn_items = [it for it in items if it.kind in fn_kinds]
    by_qual = defaultdict(list)
    for it in fn_items:
        by_qual[it.qual].append(it)
    removed = set()
    for qual, lst in by_qual.items():
        decl_only = [it for it in lst if not it.has_body and not it.pure and not it.defaulted]
        def_only = [it for it in lst if it.has_body and all(l[2] == "def" for l in it.locs)]
        for d in decl_only:
            cands = [x for x in def_only if id(x) not in removed and len(x.sig or ()) == len(d.sig or ())
                     and x.method_const == d.method_const]
            if len(cands) == 1:
                x = cands[0]
                d.locs += x.locs
                d.has_body = True
                d.notes.append("definition spells parameters as (%s)" % x.params)
                removed.add(id(x))
    items = [it for it in items if id(it) not in removed]
    # sort locs: header first
    for it in items:
        it.locs.sort(key=lambda l: (0 if l[0].endswith(".h") else 1, l[0], l[1]))
        if it.kind in fn_kinds:
            it.nparams = len(it.sig or ())
    return items


# ---------------------------------------------------------------------------
# Rust side
# ---------------------------------------------------------------------------

RUST_ITEM_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|unsafe|async|extern\s+\"C\")\s+)*"
    r"(fn|struct|enum|trait|type|const|static|impl|mod|macro_rules!)\s*([A-Za-z_][A-Za-z0-9_]*)?")


def snake(name):
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    return s.lower()


class RustFile:
    def __init__(self, path):
        self.path = path
        self.name = os.path.basename(path)
        with open(path, encoding="utf-8") as f:
            self.lines = f.read().split("\n")
        self.fns = defaultdict(list)  # name -> [(line, impl_type)]
        self.types = {}  # name -> line
        self.consts = {}
        self.trait_impls = defaultdict(list)  # type -> [(trait, line)]
        self.derives = defaultdict(set)  # type -> traits
        self.enum_variants = defaultdict(list)
        self._scan()

    def _scan(self):
        depth = 0
        impl_stack = []  # (depth_at_open, type)
        pending_impl = None
        pending_derive = set()
        cur_enum = None
        in_block_comment = False
        for ln, raw in enumerate(self.lines, 1):
            line = raw
            # strip comments (approximate; strings with // are rare in code lines)
            if in_block_comment:
                if "*/" in line:
                    line = line.split("*/", 1)[1]
                    in_block_comment = False
                else:
                    continue
            line = re.sub(r'"(?:\\.|[^"\\])*"', '""', line)
            line = re.sub(r"'(?:\\.|[^'\\])'", "' '", line)
            if "//" in line:
                line = line.split("//", 1)[0]
            if "/*" in line:
                line, _, rest = line.partition("/*")
                if "*/" not in rest:
                    in_block_comment = True
            m = re.match(r"\s*#\[derive\(([^)]*)\)\]", line)
            if m:
                pending_derive |= set(x.strip() for x in m.group(1).split(","))
            m = RUST_ITEM_RE.match(line)
            impl_type = impl_stack[-1][1] if impl_stack else None
            if m:
                kw, nm = m.group(1), m.group(2)
                if kw == "fn" and nm:
                    self.fns[nm].append((ln, impl_type))
                elif kw in ("struct", "enum", "trait", "type") and nm:
                    self.types.setdefault(nm, ln)
                    if pending_derive and kw in ("struct", "enum"):
                        self.derives[nm] |= pending_derive
                    if kw == "enum":
                        cur_enum = (nm, depth)
                elif kw in ("const", "static") and nm:
                    self.consts.setdefault(nm, ln)
                    if impl_type:
                        self.consts.setdefault(impl_type + "::" + nm, ln)
                elif kw == "impl":
                    mm = re.match(r"\s*impl(?:<[^>]*>)?\s+(?:([\w:]+)(?:<[^>]*>)?\s+for\s+)?([\w:]+)", line)
                    if mm:
                        typ = mm.group(2).split("::")[-1]
                        pending_impl = typ
                        if mm.group(1):
                            self.trait_impls[typ].append((mm.group(1).split("::")[-1], ln))
                if kw not in ("struct", "enum") or ";" in line:
                    pass
            if not m or m.group(1) != "fn":
                if not re.match(r"\s*#", line) and line.strip():
                    if not m or m.group(1) not in ("struct", "enum"):
                        pass
            if m and m.group(1) in ("struct", "enum", "fn", "type", "const", "static", "trait"):
                pending_derive = set()
            if cur_enum and depth == cur_enum[1] + 1:
                mm = re.match(r"\s*([A-Z][A-Za-z0-9_]*)\s*(?:[=,({]|$)", line)
                if mm:
                    self.enum_variants[cur_enum[0]].append((mm.group(1), ln))
            for ch in line:
                if ch == "{":
                    depth += 1
                    if pending_impl is not None:
                        impl_stack.append((depth, pending_impl))
                        pending_impl = None
                elif ch == "}":
                    if impl_stack and impl_stack[-1][0] == depth:
                        impl_stack.pop()
                    if cur_enum and depth == cur_enum[1] + 1:
                        cur_enum = None
                    depth -= 1
            if pending_impl is not None and line.rstrip().endswith(";"):
                pending_impl = None

    def next_item(self, ln):
        """Name of the next Rust item at or after 1-based line ln."""
        for k in range(ln - 1, min(len(self.lines), ln + 25)):
            s = self.lines[k]
            if re.match(r"\s*(//|#\[|$)", s):
                continue
            m = RUST_ITEM_RE.match(s)
            if m:
                return "%s %s" % (m.group(1), m.group(2) or "")
            mm = re.match(r"\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:[:=,({]|$)", s)
            if mm:
                return mm.group(1)
            return None
        return None


class Ref:
    def __init__(self, rfile, line, text, raw, mention):
        self.rfile = rfile
        self.line = line
        self.text = text
        self.raw = raw
        self.mention = mention  # prose mention ("not ported") rather than a mapping
        self.targets = []  # (Item, how)
        self.errors = []
        self.next_item = None


NEG_RE = re.compile(r"not ported|never (?:called|defined|used)|declared but|is logging-only|unused|not needed|"
                    r"debug code output|debug output", re.I)


def collect_rust_refs(rust_files):
    refs = []
    for rf in rust_files.values():
        lines = rf.lines
        for ln, s in enumerate(lines, 1):
            idx = s.find("upstream:")
            if idx < 0 or "//" not in s[:idx]:
                continue
            if s[:idx].count("`") % 2 == 1:
                continue  # example inside backticks (module docs), not a reference
            body = s[idx + len("upstream:"):]
            prose = s[:idx].rstrip().endswith("(")
            # join continuation lines while backticks/parens are unbalanced
            k = ln
            joined = body
            while k < len(lines) and k < ln + 3 and (joined.count("`") % 2 == 1 or
                                                     joined.count("(") > joined.count(")") + (1 if prose else 0)):
                nxt = lines[k]
                m = re.match(r"\s*//[/!]?\s?(.*)", nxt)
                if not m:
                    break
                joined += " " + m.group(1)
                k += 1
            ctx = joined
            # a few more lines of context to detect "not ported"
            for kk in range(k, min(len(lines), k + 2)):
                m = re.match(r"\s*//[/!]?\s?(.*)", lines[kk])
                if not m:
                    break
                ctx += " " + m.group(1)
            text = joined.strip()
            mention = bool(prose and NEG_RE.search(ctx)) or bool(re.search(r"—.*not\s+ported|— debug", ctx))
            r = Ref(rf.name, ln, text, s.strip(), mention)
            r.prose = prose
            r.next_item = rf.next_item(k + 1)
            refs.append(r)
    return refs


REF_FILE_RE = re.compile(r"^`?([a-z]+\.(?:h|cpp))\b`?\s*(.*)$")


def split_ref_names(rest):
    """Split the part after the file name into name specs."""
    rest = rest.strip()
    # cut prose after an em dash / ' -- ' / '; ' / ' is '
    rest = re.split(r"\s+—|\s+--\s|\s+-\s|;\s|\s+is\s+", rest)[0]
    rest = rest.strip().strip("`").strip()
    rest = re.sub(r"`", "", rest)
    rest = re.sub(r"[.,:]+$", "", rest).strip()
    # drop trailing prose annotations in parens that are preceded by a space
    # e.g. '(static)', '(static inline)', '(macro)', '(as patched ...)'
    while True:
        m = re.match(r"^(.*\S)\s+\(([^()]*)\)?$", rest)
        if m and not re.match(r"^\s*$", m.group(2) or "") and " " not in m.group(1)[-1:]:
            # only strip if the paren content is prose (contains a letter word w/o '::')
            if re.match(r"^(static|static inline|macro|as patched|private members|together with|needed by|"
                        r"logging|unused)", m.group(2) or ""):
                rest = m.group(1)
                continue
        break
    m = re.match(r"^(.*?)\s+\((?:static|static inline|macro|as patched|private members|together with|needed by).*$", rest)
    if m:
        rest = m.group(1)
    # split on ' / ' and on '/' between names
    parts = []
    for p in re.split(r"\s+/\s+", rest):
        p = p.strip()
        if "/" in p and "operator/" not in p:
            subs = p.split("/")
            parts.append(subs[0])
            for s2 in subs[1:]:
                parts.append(("SLASH", subs[0], s2))
        else:
            parts.append(p)
    return parts


def parse_spec(spec):
    """Return dict(kind_kw, name, args, const, wildcard, range)."""
    s = spec.strip()
    d = dict(kw=None, name=None, args=None, const=False, wildcard=False, rng=None)
    m = re.match(r"^#define\s+(\w+)", s)
    if m:
        d["kw"] = "define"
        d["name"] = m.group(1)
        return d
    m = re.match(r"^(class|struct|enum|typedef|static const \w+|const \w+)\s+(.*)$", s)
    if m:
        d["kw"] = m.group(1).split()[0]
        s = m.group(2).strip()
        if d["kw"] == "typedef":
            ids = re.findall(r"[A-Za-z_]\w*", s)
            if not ids:
                return d
            if ids[-1] == "typedefs":
                d["name"] = None
                return d
            d["name"] = ids[-1]
            return d
        if d["kw"] in ("static", "const"):
            d["name"] = re.findall(r"[A-Za-z_]\w*", s)[0]
            return d
    m = re.match(r"^(\w+)\s*\.\.\.\s*(\w+)", s)
    if m:
        d["rng"] = (m.group(1), m.group(2))
        return d
    # name with optional args
    m = re.match(r"^((?:[A-Za-z_~]\w*::)*(?:operator\s*(?:\(\)|\[\]|[^\s(]+)|~?[A-Za-z_]\w*\*?))\s*(\((.*?)\))?\s*(const)?(.*)$", s)
    if not m:
        return d
    d["name"] = m.group(1).replace(" ", "")
    if d["name"].endswith("*"):
        d["wildcard"] = True
    if m.group(2) is not None:
        d["args"] = m.group(3)
    d["const"] = bool(m.group(4))
    trail = (m.group(5) or "").strip()
    if trail.startswith("::"):
        # e.g. "class ANodeCmp::operator()" handled above; nothing else
        pass
    return d


def args_loose(argstr):
    if argstr is None:
        return None
    a = argstr.strip()
    if a in ("", "void"):
        return ()
    out = []
    for p in split_args(a):
        p = re.sub(r"=.*$", "", p).strip()
        toks = re.findall(r"::|[A-Za-z_]\w*|[*&<>,]|\S", p)
        fake = [Tok("id" if re.match(r"[A-Za-z_]", x) else "p", x, 0, None) for x in toks]
        tt = param_type_tokens(fake)
        out.append(loose_type([t.text for t in tt]))
    return tuple(out)


def split_args(a):
    parts, cur, d = [], "", 0
    for ch in a:
        if ch in "(<[":
            d += 1
        elif ch in ")>]":
            d -= 1
        if ch == "," and d == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    return parts


# ---------------------------------------------------------------------------
# Mapping
# ---------------------------------------------------------------------------

FN_KINDS = ("method", "ctor", "dtor", "operator", "function")
TYPE_KINDS = ("class", "struct", "union", "enum", "typedef")


class Index:
    def __init__(self, items, files):
        self.items = items
        self.files = set(files)
        self.by_qual = defaultdict(list)
        self.by_name = defaultdict(list)
        self.by_enumerator = defaultdict(list)
        for it in items:
            self.by_qual[it.qual].append(it)
            self.by_name[it.name].append(it)
            for e in it.enumerators:
                self.by_enumerator[e].append(it)
                if "::" in it.qual:
                    self.by_enumerator[it.qual.rsplit("::", 1)[0] + "::" + e].append(it)

    def lookup(self, name):
        """All items whose qualified name is `name` or ends with '::name'."""
        res = list(self.by_qual.get(name, []))
        if not res and "::" not in name:
            res = [it for it in self.by_name.get(name, [])]
        if not res:
            res = [it for it in self.items if it.qual.endswith("::" + name)]
        enum_hits = self.by_enumerator.get(name, [])
        return res, enum_hits


def item_in_stem(it, stem):
    return it.stem == stem or any(l[0].rsplit(".", 1)[0] == stem for l in it.locs)


def resolve_spec(index, stem, spec, kw_hint=None):
    """Return (list of (item, how), error-or-None, ambiguous_overload_set)."""
    d = parse_spec(spec)
    name = d["name"]
    if d["rng"]:
        a, b = d["rng"]
        ra, _ = index.lookup(a)
        rb, _ = index.lookup(b)
        ea = index.by_enumerator.get(a, [])
        if ea:
            return [(it, "comment") for it in ea], None, False
        ra = [x for x in ra if item_in_stem(x, stem)]
        rb = [x for x in rb if item_in_stem(x, stem)]
        if not ra or not rb:
            return [], "range end not found", False
        lo, hi = ra[0].line, rb[0].line
        f = ra[0].file
        hits = [it for it in index.items if it.file == f and lo <= it.line <= hi and it.kind in ("constant", "enum", "define")]
        return [(it, "comment") for it in hits], None, False
    if spec.strip() == "typedefs":
        # "<stem>.h typedefs": the header's typedefs that the referencing
        # Rust file defines as `type` aliases.
        rtypes = RUST_TYPES_OF_REF.get(CURRENT_REF_FILE[0], {})
        hits = [it for it in index.items if it.kind == "typedef" and it.file == stem + ".h"
                and it.name in rtypes]
        if not hits:
            return [], "no typedef of %s.h is defined in the referencing Rust file" % stem, False
        return [(it, "comment (typedefs block)") for it in hits], None, False
    if name is None:
        if d["kw"] == "typedef":
            hits = [it for it in index.items if it.kind == "typedef" and item_in_stem(it, stem)]
            return [(it, "comment (all typedefs of %s.h)" % stem) for it in hits], None, False
        return [], "unparsable reference", False
    if d["wildcard"]:
        pat = name
        hits = [it for it in index.items if item_in_stem(it, stem) and fnmatch.fnmatchcase(it.qual, pat)
                and it.kind in ("constant", "define", "enum", "variable")]
        ehits = []
        for it in index.items:
            if item_in_stem(it, stem) and any(fnmatch.fnmatchcase(e, pat.split("::")[-1]) for e in it.enumerators):
                ehits.append(it)
        hits = hits + [e for e in ehits if e not in hits]
        if not hits:
            return [], "wildcard matches nothing", False
        return [(it, "comment") for it in hits], None, False
    res, enum_hits = index.lookup(name)
    if d["kw"] in ("class", "struct"):
        res = [x for x in res if x.kind in ("class", "struct", "union")] or res
    elif d["kw"] == "enum":
        res = [x for x in res if x.kind == "enum"] or res
    elif d["kw"] == "typedef":
        res = [x for x in res if x.kind in ("typedef", "enum", "struct", "class")] or res
    elif d["kw"] == "define":
        res = [x for x in res if x.kind in ("define", "macro")] or res
    elif d["args"] is not None:
        fres = [x for x in res if x.kind in FN_KINDS or x.kind == "macro"]
        res = fres or res
    if not res and enum_hits:
        res = enum_hits
    if not res:
        return [], "no upstream item named %s" % name, False
    in_stem = [x for x in res if item_in_stem(x, stem)]
    if not in_stem:
        where = sorted(set(x.file for x in res))
        return [], "%s is not in %s.{h,cpp} (found in %s)" % (name, stem, ", ".join(where)), False
    res = in_stem
    fns = [x for x in res if x.kind in FN_KINDS]
    if len(fns) > 1:
        # overload resolution
        if d["const"]:
            c = [x for x in fns if x.method_const]
            if c:
                fns = c
        elif len(set(x.sig for x in fns)) == 1:
            # const / non-const pair with identical parameters: an
            # unqualified reference names the non-const one.
            nc = [x for x in fns if not x.method_const]
            if len(nc) == 1:
                fns = nc
        elif d["args"] is not None:
            nc = [x for x in fns if not x.method_const]
            if nc and len(nc) < len(fns) and len(set(x.sig for x in nc)) == len(nc) and \
                    any(x.sig == args_loose(d["args"]) for x in nc):
                fns = nc
        if len(fns) > 1 and d["args"] is not None and re.search(r",\s*\.\.\.?\s*$", d["args"]):
            # `f(Type *, ..)`: leading parameter types only
            al = args_loose(re.sub(r",\s*\.\.\.?\s*$", "", d["args"]))
            pre = [x for x in fns if tuple(x.sig or ())[:len(al)] == al]
            if len(pre) == 1:
                return [(pre[0], "comment")], None, False
        if len(fns) > 1 and d["args"] is not None:
            al = args_loose(d["args"])
            exact = [x for x in fns if x.sig == al]
            if len(exact) == 1:
                return [(exact[0], "comment")], None, False
            if al == ():
                zero = [x for x in fns if x.nparams == 0]
                if len(zero) == 1:
                    # `()` is also used as "unspecified"; accept the 0-arg overload
                    return [(zero[0], "comment")], None, False
            else:
                cnt = [x for x in fns if x.nparams == len(al)]
                if len(cnt) == 1:
                    return [(cnt[0], "comment (by arity)")], None, False
            return [(x, "comment (overload set)") for x in fns], None, True
        if len(fns) > 1:
            return [(x, "comment (overload set)") for x in fns], None, True
        return [(fns[0], "comment")], None, False
    return [(x, "comment") for x in res], None, False


RUST_TYPES_OF_REF = {}
CURRENT_REF_FILE = [None]


def resolve_ref(index, ref):
    CURRENT_REF_FILE[0] = ref.rfile
    text = ref.text
    # several file-qualified refs separated by ', '
    chunks = re.split(r",\s+(?=[a-z]+\.(?:h|cpp)\b)", text)
    out = []
    errors = []
    ambiguous = False
    any_file = False
    for ch in chunks:
        m = REF_FILE_RE.match(ch.strip().strip("`"))
        if not m:
            continue
        any_file = True
        fname, rest = m.group(1), m.group(2)
        stem = fname.rsplit(".", 1)[0]
        if fname not in index.files and not os.path.exists(os.path.join(UPSTREAM, fname)):
            errors.append("upstream file %s does not exist" % fname)
            continue
        specs = split_ref_names(rest)
        prev_full = None
        for sp in specs:
            if isinstance(sp, tuple):
                _, first, suffix = sp
                suffix = suffix.strip()
                cands = []
                if "::" in first and "::" not in suffix:
                    cands.append(first.rsplit("::", 1)[0] + "::" + suffix)
                fn = re.match(r"^[\w:~]+", first)
                fbase = fn.group(0) if fn else first
                if len(suffix) < len(fbase) and re.match(r"^\w+$", suffix):
                    cands.append(fbase[:-len(suffix)] + suffix)
                cands.append(suffix)
                done = False
                for c in cands:
                    hits, err, amb = resolve_spec(index, stem, c)
                    if hits:
                        out += hits
                        ambiguous |= amb
                        done = True
                        break
                if not done:
                    errors.append("cannot resolve '%s' (after '/')" % suffix)
                continue
            sp2 = sp
            if prev_full and "::" in prev_full and re.match(r"^(operator|~)", sp) and "::" not in sp:
                sp2 = prev_full.rsplit("::", 1)[0] + "::" + sp
            hits, err, amb = resolve_spec(index, stem, sp2)
            if not hits and err and sp2 != sp:
                hits, err, amb = resolve_spec(index, stem, sp)
            if err:
                errors.append(err)
            out += hits
            ambiguous |= amb
            nm = parse_spec(sp2)["name"]
            if nm:
                prev_full = nm
    if not any_file:
        # `Class::method()` without a file name: resolve over all files and
        # report the missing file name (non-fatal).
        m = re.match(r"^`?((?:[A-Za-z_]\w*::)+~?[A-Za-z_]\w*)\s*\(([^()]*)\)", text)
        if not m:
            return None  # free-form prose, not a reference
        res, _ = index.lookup(m.group(1))
        if not res:
            ref.targets = []
            ref.errors = ["no upstream item named %s (and no file name given)" % m.group(1)]
            ref.ambiguous = False
            return ref
        stems = sorted(set(x.stem for x in res))
        hits, err, amb = resolve_spec(index, stems[0], text.split(" ")[0].strip("`") if "(" in text.split(" ")[0]
                                      else m.group(0))
        ref.targets = [(it, how) for it, how in hits]
        ref.errors = []
        ref.ambiguous = amb
        ref.nofile = sorted(set(l[0] for it, _ in hits for l in it.locs))
        if getattr(ref, "prose", False):
            ref.mention = True
        return ref
    ref.targets = out
    ref.errors = errors
    ref.ambiguous = ambiguous
    return ref


STD_TRAITS = {
    "operator==": ("PartialEq", "Eq"),
    "operator!=": ("PartialEq", "Eq"),
    "operator<": ("PartialOrd", "Ord"),
    "operator+": ("Add",),
    "operator-": ("Sub",),
    "operator[]": ("Index", "IndexMut"),
    "operator<<": ("Display", "Debug"),
}


def name_map(it, rust_files):
    """Secondary mapping by name.  Returns (how, location) or None."""
    rf = rust_files.get(it.stem + ".rs")
    files = [rf] if rf else []
    name = it.name
    if it.kind in TYPE_KINDS:
        for f in files:
            if name in f.types:
                return "name (type)", "%s:%d" % (f.name, f.types[name])
        return None
    if it.kind in ("constant", "define", "variable"):
        for f in files:
            for cand in (name, snake(name).upper()):
                if cand in f.consts:
                    return "name (const)", "%s:%d" % (f.name, f.consts[cand])
        return None
    if it.kind == "operator":
        cls = it.owner_class.split("::")[-1] if it.owner_class else None
        traits = STD_TRAITS.get(name)
        if traits and cls:
            for f in rust_files.values():
                for tr, ln in f.trait_impls.get(cls, []):
                    if tr in traits:
                        return "name (impl %s)" % tr, "%s:%d" % (f.name, ln)
                if f.derives.get(cls, set()) & set(traits):
                    return "name (derive %s)" % "/".join(sorted(f.derives[cls] & set(traits))), \
                        "%s:%d" % (f.name, f.types.get(cls, 0))
        return None
    if it.kind in ("method", "function"):
        sn = snake(name)
        for f in files:
            hits = f.fns.get(sn) or f.fns.get(name)
            if hits:
                cls = it.owner_class.split("::")[-1] if it.owner_class else None
                pref = [h for h in hits if h[1] == cls] or hits
                return "name (fn %s)" % sn, "%s:%d" % (f.name, pref[0][0])
        return None
    return None


# ---------------------------------------------------------------------------
# Classification file
# ---------------------------------------------------------------------------

CATEGORIES = {"debug-output", "timer", "dead-code", "declared-undefined", "subsumed",
              "not-applicable", "MISSING", "ported"}


class Cls:
    def __init__(self, line, spec, category, reason):
        self.line = line
        self.spec = spec
        self.category = category
        self.reason = reason
        self.hits = 0


def load_classification():
    out = []
    errs = []
    if not os.path.exists(CLASSIFICATION):
        return out, errs
    with open(CLASSIFICATION, encoding="utf-8") as f:
        for ln, s in enumerate(f, 1):
            s = s.rstrip("\n")
            if not s.strip() or s.lstrip().startswith("#"):
                continue
            parts = [p.strip() for p in s.split("|")]
            if len(parts) != 3 or not parts[2]:
                errs.append("classification.txt:%d: expected 'file item | category | reason'" % ln)
                continue
            if parts[1] not in CATEGORIES:
                errs.append("classification.txt:%d: unknown category %r" % (ln, parts[1]))
                continue
            out.append(Cls(ln, parts[0], parts[1], parts[2]))
    return out, errs


def cls_matches(c, it):
    m = re.match(r"^(\S+)\s+(.*)$", c.spec)
    if not m:
        return False
    f, name = m.group(1), m.group(2).strip()
    fstem = f.rsplit(".", 1)[0] if "." in f else f
    if not (f == it.file or f == fstem and item_in_stem(it, fstem) or any(l[0] == f for l in it.locs)
            or ("." in f and item_in_stem(it, fstem) and False)):
        if not (f.endswith((".h", ".cpp")) and item_in_stem(it, fstem)):
            return False
    core = name.replace("operator()", "operator")
    if "(" in core and not name.startswith("enum {"):
        disp = it.display()
        return fnmatch.fnmatchcase(disp, name) or disp == name
    return fnmatch.fnmatchcase(it.qual, name.replace("[", "[[]"))


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def fixture_index():
    with open(FIXTURES_OPS, encoding="utf-8") as f:
        src = f.read()
    m = re.search(r"fixtures!\s*\{(.*?)\n\}", src, re.S)
    status = {}
    if m:
        for mm in re.finditer(r"(?m)^\s*(golden|smoke|slow)\s+(\w+)(?:\s*=\s*\"([^\"]+)\")?\s*;", m.group(1)):
            status[mm.group(3) or mm.group(2)] = mm.group(1)
    # `slow` only gates the run; the op script's `mode` says whether it is
    # compared with a golden (golden) or only run (smoke).
    for name, kind in list(status.items()):
        if kind != "slow":
            continue
        mode = "golden"
        ops = os.path.join(ROOT, "tests", "fixtures", name + ".ops")
        if os.path.exists(ops):
            with open(ops, encoding="utf-8") as f:
                for line in f:
                    if line.startswith("mode "):
                        mode = line.split()[1]
                        break
        status[name] = "slow " + mode
    tests_dir = os.path.join(UPSTREAM, "tests")
    fixtures = sorted(f[:-4] for f in os.listdir(tests_dir) if f.endswith(".cpp"))
    return fixtures, status


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def build():
    files, raw, warnings = parse_upstream()
    items = merge_items(raw)
    rust_files = OrderedDict()
    for f in sorted(os.listdir(RUST_DIR)):
        if f.endswith(".rs"):
            rust_files[f] = RustFile(os.path.join(RUST_DIR, f))
    index = Index(items, files)
    refs = collect_rust_refs(rust_files)
    for f in rust_files.values():
        RUST_TYPES_OF_REF[f.name] = f.types
    bad_refs = []
    ambiguous_refs = []
    for r in refs:
        res = resolve_ref(index, r)
        if res is None:
            continue
        if r.errors:
            bad_refs.append(r)
        if getattr(r, "ambiguous", False) and not r.mention:
            ambiguous_refs.append(r)
        for it, how in r.targets:
            loc = "%s:%d" % (r.rfile, r.line)
            if r.mention:
                it.notes.append("mentioned at %s" % loc)
                it.mentions = getattr(it, "mentions", []) + [loc]
            else:
                it.rust.append((how, loc, r.next_item))
                if how.endswith("(overload set)"):
                    it.overload_ambiguous = True
    # classification
    classes, cerrs = load_classification()
    conflicts = []
    for it in items:
        mapped = [x for x in it.rust if not x[0].endswith("(overload set)")]
        cl = [c for c in classes if cls_matches(c, it)]
        for c in cl:
            c.hits += 1
        if it.rust:
            it.status = "ported"
            it.method = "comment"
            if cl and cl[0].category not in ("subsumed",):
                conflicts.append((it, cl[0]))
            if cl:
                it.reason = cl[0].reason
            continue
        if cl:
            c = cl[0]
            if c.category == "ported":
                it.status = "ported"
                it.rust.append(("manual", c.reason, None))
                continue
            if c.category == "MISSING":
                it.status = "MISSING"
            else:
                it.status = "not-ported"
            it.category = c.category
            it.reason = c.reason
            continue
        nm = name_map(it, rust_files)
        if nm:
            it.status = "ported"
            it.rust.append((nm[0], nm[1], None))
            continue
        it.status = None  # unclassified
    stale = [c for c in classes if c.hits == 0]
    unclassified = [it for it in items if it.status is None]
    fixtures, fstatus = fixture_index()
    return dict(files=files, items=items, refs=refs, bad_refs=bad_refs, ambiguous_refs=ambiguous_refs,
                warnings=warnings, cerrs=cerrs, stale=stale, unclassified=unclassified,
                conflicts=conflicts, fixtures=fixtures, fstatus=fstatus, rust_files=rust_files)


def kind_label(it):
    k = it.kind
    extra = []
    if it.kind in FN_KINDS:
        if "static" in it.flags and not it.owner_class:
            extra.append("static")
        if "inline" in it.flags:
            extra.append("inline")
        if "virtual" in it.flags:
            extra.append("virtual")
        if it.pure:
            extra.append("pure")
        if "friend" in it.flags:
            extra.append("friend")
        if "template" in it.flags:
            extra.append("template")
        if "static" in it.flags and it.owner_class:
            extra.append("static member")
    return k + (" (" + ", ".join(extra) + ")" if extra else "")


def md_escape(s):
    return s.replace("|", "\\|").replace("<", "&lt;").replace(">", "&gt;")


def item_cell(it):
    s = it.display()
    if it.kind == "enum" and it.enumerators:
        s += ": " + ", ".join(it.enumerators)
    return "`" + s.replace("`", "") .replace("|", "\\|") + "`"


def where_cell(it):
    locs = []
    for f, ln, what, cond in it.locs:
        locs.append("%s:%d%s" % (f, ln, "" if what == "def" or it.kind not in FN_KINDS else " (decl)"))
    seen = []
    for l in locs:
        if l not in seen:
            seen.append(l)
    return "<br>".join(seen)


def status_cell(it):
    if it.status == "ported":
        how = sorted(set(re.sub(r" \(.*", "", x[0]) for x in it.rust))
        s = "ported (" + ", ".join(how) + ")"
        if it.overload_ambiguous and not any(not x[0].endswith("(overload set)") for x in it.rust):
            s += " [overload not individually mapped]"
        return s
    if it.status == "not-ported":
        return "not-ported: " + it.category
    return it.status or "UNCLASSIFIED"


def rust_cell(it):
    if it.status == "ported":
        parts = []
        seen = set()
        for how, loc, nxt in it.rust:
            if loc in seen:
                continue
            seen.add(loc)
            if how == "manual":
                parts.append(md_escape(loc))
                continue
            parts.append("%s%s" % (loc, (" `" + nxt.strip() + "`") if nxt else ""))
        s = "<br>".join(parts[:4]) + (" (+%d more)" % (len(parts) - 4) if len(parts) > 4 else "")
        if it.reason and not any(x[0] == "manual" for x in it.rust):
            s += "<br>" + md_escape(it.reason)
        return s
    return md_escape(it.reason)


def summarize(items):
    per = OrderedDict()
    for it in items:
        per.setdefault(it.stem, []).append(it)
    return per


def write_md(B):
    items = B["items"]
    lines = []
    w = lines.append
    w("# libavoid Porting Inventory")
    w("")
    w("<!-- GENERATED FILE: do not edit by hand.  Regenerate with `tools/audit/inventory.py`. -->")
    w("")
    w("Function-level inventory of upstream libavoid (`vendor/adaptagrams/cola/libavoid`, excluding")
    w("`tests/` and `dllexport.h`) against the Rust port in `src/avoid/`.")
    w("")
    w("* **Generated** by `python3 tools/audit/inventory.py`, which parses the upstream sources (own")
    w("  tokenizer, preprocessor and scope-tracking declaration parser), maps every item to Rust via the")
    w("  `// upstream: <file> <item>` comments (primary) or by name (secondary: camelCase -> snake_case `fn`,")
    w("  same-named type/const, operator -> std trait impl/derive), and applies the manual")
    w("  classifications in `tools/audit/classification.txt`.")
    w("* **Check** with `python3 tools/audit/inventory.py --check`: exits non-zero if a Rust `upstream:`")
    w("  comment names a nonexistent upstream file/item, if an upstream item is neither mapped nor")
    w("  classified, if a classification entry is stale, or if a classification contradicts a comment")
    w("  mapping.")
    w("* **Statuses**: `ported` (with mapping method), `not-ported: <category>` (`debug-output`, `timer`,")
    w("  `dead-code`, `declared-undefined`, `subsumed`, `not-applicable`; each with a reason), `MISSING`")
    w("  (genuinely absent from the port; see the reason column).")
    w("* Items in inactive preprocessor regions are listed with their condition, e.g. `dead (#if 0)`.")
    w("* Header declarations and .cpp definitions are merged; *Upstream* lists all locations, `(decl)`")
    w("  marks a declaration without body.")
    w("")
    # summary
    w("## Summary")
    w("")
    cats = ["debug-output", "timer", "dead-code", "declared-undefined", "subsumed", "not-applicable"]
    w("| Upstream file | Items | Ported | " + " | ".join("NP: " + c for c in cats) + " | MISSING |")
    w("| --- | ---: | ---: | " + " | ".join("---:" for _ in cats) + " | ---: |")
    per = summarize(items)
    tot = defaultdict(int)
    for stem in sorted(per):
        lst = per[stem]
        fnames = sorted(set(l[0] for it in lst for l in it.locs if l[0].rsplit(".", 1)[0] == stem))
        row = [len(lst), sum(1 for it in lst if it.status == "ported")]
        row += [sum(1 for it in lst if it.status == "not-ported" and it.category == c) for c in cats]
        row += [sum(1 for it in lst if it.status == "MISSING")]
        keys = ["items", "ported"] + cats + ["MISSING"]
        for k, v in zip(keys, row):
            tot[k] += v
        w("| [%s](#%s) | %s |" % ("/".join(fnames) or stem, stem, " | ".join(str(x) for x in row)))
    w("| **total** | %s |" % " | ".join("**%d**" % tot[k] for k in ["items", "ported"] + cats + ["MISSING"]))
    w("")
    meth = defaultdict(int)
    for it in items:
        if it.status == "ported":
            if any(x[0].startswith("comment") for x in it.rust):
                meth["comment"] += 1
            elif any(x[0] == "manual" for x in it.rust):
                meth["manual"] += 1
            else:
                meth["name"] += 1
    w("Ported items by mapping method: `upstream:` comment %d, name match %d, manual mapping in "
      "`classification.txt` %d." % (meth["comment"], meth["name"], meth["manual"]))
    w("")
    missing = [it for it in items if it.status == "MISSING"]
    if missing:
        w("### MISSING items")
        w("")
        w("| Upstream item | Kind | Upstream | Description |")
        w("| --- | --- | --- | --- |")
        for it in missing:
            w("| %s | %s | %s | %s |" % (item_cell(it), kind_label(it), where_cell(it), md_escape(it.reason)))
        w("")
    amb = [it for it in items if it.overload_ambiguous and not any(not x[0].endswith("(overload set)") for x in it.rust)]
    if amb:
        w("### Overloads mapped only as a set")
        w("")
        w("These overloads are referenced by an `upstream:` comment that does not disambiguate the overload.")
        w("")
        for it in amb:
            w("* %s (%s)" % (item_cell(it), where_cell(it)))
        w("")
    w("## Per-file inventory")
    w("")
    for stem in sorted(per):
        w('<a id="%s"></a>' % stem)
        w("### %s" % stem)
        w("")
        w("| Upstream item | Kind | Upstream | Status | Rust location / reason |")
        w("| --- | --- | --- | --- | --- |")
        for it in sorted(per[stem], key=lambda x: (0 if x.file.endswith(".h") else 1, x.file, x.line)):
            kl = kind_label(it)
            if it.cond:
                kl += " — " + it.cond
            w("| %s | %s | %s | %s | %s |" % (item_cell(it), kl, where_cell(it), status_cell(it), rust_cell(it)))
        w("")
    # fixtures
    w("## Upstream Fixture Index")
    w("")
    w("Generated from `vendor/adaptagrams/cola/libavoid/tests/*.cpp` and the `fixtures!` list in")
    w("`tests/fixtures_ops.rs` (op scripts in `tests/fixtures/*.ops`):")
    w("`golden` = ops-driven, compared byte-for-byte against `tests/golden-bump/`; `smoke` = does not")
    w("compile against the current upstream API, run as a smoke test only; `slow golden` / `slow smoke`")
    w("= the same, gated behind `LIBAVOID_SLOW_FIXTURES=1` (upstream takes minutes).  Status of the")
    w("last audit: every golden fixture passes (`cargo test --release`, `tools/fixtures/run_all.sh`).")
    w("")
    fs = B["fstatus"]
    counts = defaultdict(int)
    for f in B["fixtures"]:
        counts[fs.get(f, "NOT IN fixtures_ops.rs")] += 1
    w(", ".join("%s: %d" % (k, v) for k, v in sorted(counts.items())) + " (of %d upstream fixtures)." % len(B["fixtures"]))
    w("")
    w("| Fixture | Status |")
    w("| --- | --- |")
    for f in B["fixtures"]:
        w("| `%s.cpp` | %s |" % (f, fs.get(f, "**not in tests/fixtures_ops.rs**")))
    extra = sorted(set(fs) - set(B["fixtures"]))
    if extra:
        w("")
        w("Entries in `tests/fixtures_ops.rs` without an upstream fixture: " + ", ".join("`%s`" % x for x in extra))
    w("")
    with open(OUTPUT, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


def report_problems(B, out=sys.stderr):
    bad = 0
    for wmsg in B["warnings"]:
        print("parser warning: " + wmsg, file=out)
    for e in B["cerrs"]:
        print(e, file=out)
        bad += 1
    if B["bad_refs"]:
        print("\n(a) bad Rust `upstream:` references:", file=out)
        for r in B["bad_refs"]:
            print("  src/avoid/%s:%d: %s\n      -> %s" % (r.rfile, r.line, r.text, "; ".join(r.errors)), file=out)
            bad += 1
    nofile = [r for r in B["refs"] if getattr(r, "nofile", None) and not r.errors]
    if nofile:
        print("\nnote (non-fatal): `upstream:` references without a file name:", file=out)
        for r in nofile:
            print("  src/avoid/%s:%d: %s  (item is in %s)" % (r.rfile, r.line, r.text[:80], ", ".join(r.nofile)),
                  file=out)
    if B["unclassified"]:
        print("\n(b) upstream items neither mapped nor classified:", file=out)
        for it in B["unclassified"]:
            print("  %s:%d %s [%s]%s" % (it.file, it.line, it.display(), it.kind,
                                        (" " + it.cond) if it.cond else ""), file=out)
            bad += 1
    if B["stale"]:
        print("\n(c) stale classification entries:", file=out)
        for c in B["stale"]:
            print("  classification.txt:%d: %s" % (c.line, c.spec), file=out)
            bad += 1
    if B["conflicts"]:
        print("\n(d) classified as not ported/MISSING but mapped by an upstream: comment:", file=out)
        for it, c in B["conflicts"]:
            print("  classification.txt:%d %s -> %s" % (c.line, it.display(), it.rust[0][1]), file=out)
            bad += 1
    return bad


def dump(B):
    for it in B["items"]:
        print("%-16s %-12s %-6s %s%s  [%s] %s %s" % (
            "%s:%d" % (it.file, it.line), it.kind, "body" if it.has_body else ("pure" if it.pure else "NOBODY"),
            it.display(), (" {" + ",".join(it.enumerators) + "}") if it.enumerators else "",
            it.cond or "", status_cell(it), "; ".join(x[1] for x in it.rust)))


def main(argv):
    B = build()
    if "--dump" in argv:
        dump(B)
        return 0
    if "--check" in argv:
        bad = report_problems(B)
        print("inventory: %d items; %d problems" % (len(B["items"]), bad), file=sys.stderr)
        return 1 if bad else 0
    write_md(B)
    bad = report_problems(B)
    print("wrote %s (%d items; %d problems)" % (os.path.relpath(OUTPUT, ROOT), len(B["items"]), bad),
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
