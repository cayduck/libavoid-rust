#!/usr/bin/env python3
"""Converts the upstream libavoid test fixtures into line-based op scripts.

    tools/fixtures/convert.py                      # convert every fixture
    tools/fixtures/convert.py NAME...              # convert some fixtures
    tools/fixtures/convert.py --check [NAME...]    # verify tests/fixtures/*.ops are up to date
    tools/fixtures/convert.py --emit-cpp OPS OUT   # regenerate C++ from an op script

The upstream fixtures (vendor/adaptagrams/cola/libavoid/tests/*.cpp) are
mostly machine generated straight-line C++.  This script implements a small
C++ front end for exactly the subset they use: a C preprocessor (comments,
#if/#ifdef/#else/#endif/#define with the oracle's flags, i.e. no extra
defines), a tokenizer, a recursive-descent parser for declarations,
expressions, if/else and function definitions, and a lowering pass that
resolves overloads, implicit conversions and default arguments by static
type and emits one op per C++ statement.  Helper functions (`test()`, the
`outputDiagram(router)` counter helper) are inlined; their compile-time
state (static counters, ostringstreams) is evaluated during conversion.

Anything outside the understood subset raises ConvertError: nothing is ever
skipped silently.

Op script format (tests/fixtures/<name>.ops) -- one statement per line,
`#` lines are comments:

    fixture NAME              header: fixture name
    mode golden|smoke         smoke: no golden to compare with (fixture does
                              not compile upstream; scene modernised)
    let VAR TYPE EXPR         declare VAR (TYPE: router poly point connend
                              conn shape junction pin checkpoint checkpoints
                              connendlist rerouter ndlists jiter citer jcpair
                              endpair connlist int double bool)
    decl VAR TYPE             declare default-constructed VAR
    set LVALUE EXPR           assignment; LVALUE is VAR, (ps VAR i), (x VAR),
                              (y VAR), (_id VAR) or (at VAR i)
    do EXPR                   evaluate for side effects
    output ROUTER GOLDEN      router->outputDiagram(); compare with
                              tests/golden/GOLDEN.txt
    output-unchecked ROUTER GOLDEN
                              same, but a later call overwrote the golden
    delete ROUTER GOLDEN      delete router; compare final state with
                              tests/golden/GOLDEN.txt (`-` in smoke mode)
    callback CONN route|displayRoute
                              conn->setCallback(cb) where cb prints
                              "Connector %u needs rerouting!" + the route
    print "C-STRING"          printf of a literal string
    assert EXPR               assert()/COLA_ASSERT() (active: no NDEBUG)
    if EXPR / else / end      control flow
    ret EXPR                  return value of main (process exit code)

EXPR is an atom (integer or double literal kept verbatim from the source,
true, false, null, "string", VAR) or a prefix form `(FN ARG...)`; the
function names are the upstream C++ API names (see FUNCS below and the
interpreter in tests/common/fixture_ops.rs).
"""

import argparse
import hashlib
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OPS_DIR = os.path.join(ROOT, "tests", "fixtures")
GOLDEN_DIR = os.path.join(ROOT, "tests", "golden")


def upstream_dir():
    env = os.environ.get("LIBAVOID_UPSTREAM")
    if env:
        return env
    cand = os.path.join(ROOT, "vendor", "adaptagrams", "cola", "libavoid")
    if os.path.isdir(cand):
        return cand
    # In a git worktree the (gitignored) vendor tree lives in the main checkout.
    try:
        common = subprocess.check_output(
            ["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],
            text=True, stderr=subprocess.DEVNULL).strip()
        cand = os.path.join(os.path.dirname(common), "vendor", "adaptagrams", "cola", "libavoid")
        if os.path.isdir(cand):
            return cand
    except (OSError, subprocess.CalledProcessError):
        pass
    raise SystemExit("cannot find upstream libavoid sources; set LIBAVOID_UPSTREAM")


# Fixtures that do not compile against the current upstream API.  Their
# scenes are converted with the listed modernisations and run as smoke tests
# (no golden output exists).
SMOKE_FIXTURES = {
    "corneroverlap01": "uses removed Router::setOrthogonalNudgeDistance()",
    "nudgeCrossing01": "includes libcola/cola.h (libcola is not built by the oracle)",
    "reallyslowrouting": "uses removed Router::setOrthogonalNudgeDistance() and the "
                         "old Router::addShape() registration API",
}


class ConvertError(Exception):
    pass


# ---------------------------------------------------------------------------
# Preprocessor
# ---------------------------------------------------------------------------

def strip_comments(src, fname):
    """Replaces comments by spaces (keeping newlines so that line numbers
    stay valid).  Fails on a block comment that starts on a preprocessor
    directive line and spans lines (would change directive extent)."""
    out = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c == '"' or c == "'":
            q = c
            j = i + 1
            while j < n and src[j] != q:
                if src[j] == "\\":
                    j += 1
                if src[j] == "\n":
                    raise ConvertError("%s: newline in literal" % fname)
                j += 1
            out.append(src[i:j + 1])
            i = j + 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            if j < 0:
                j = n
            if src[j - 1:j] == "\\":
                raise ConvertError("%s: line comment continued with backslash" % fname)
            out.append(" ")
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            if j < 0:
                raise ConvertError("%s: unterminated block comment" % fname)
            body = src[i:j + 2]
            if "\n" in body:
                # directive line?
                k = src.rfind("\n", 0, i) + 1
                if src[k:i].lstrip().startswith("#"):
                    raise ConvertError("%s: multi-line comment on a directive line" % fname)
            out.append(" " + "\n" * body.count("\n"))
            i = j + 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


PREDEFINED = {"__cplusplus": 201103, "__GNUC__": 1, "__linux__": 1}


def eval_pp_expr(text, defines, where):
    toks = re.findall(r"\d+|[A-Za-z_]\w*|&&|\|\||==|!=|[()!]|\S", text)
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def take():
        t = peek()
        pos[0] += 1
        return t

    def primary():
        t = take()
        if t is None:
            raise ConvertError("%s: bad #if expression %r" % (where, text))
        if t == "(":
            v = orexpr()
            if take() != ")":
                raise ConvertError("%s: bad #if expression %r" % (where, text))
            return v
        if t == "!":
            return 0 if primary() else 1
        if t == "defined":
            par = peek() == "("
            if par:
                take()
            name = take()
            if par and take() != ")":
                raise ConvertError("%s: bad defined() in %r" % (where, text))
            return 1 if (name in defines or name in PREDEFINED) else 0
        if re.fullmatch(r"\d+", t):
            return int(t)
        if re.fullmatch(r"[A-Za-z_]\w*", t):
            if t in defines:
                v = defines[t]
                if v == "":
                    raise ConvertError("%s: empty macro %s used in #if" % (where, t))
                return int(v)
            if t in PREDEFINED:
                return PREDEFINED[t]
            return 0  # undefined identifiers evaluate to 0
        raise ConvertError("%s: unsupported token %r in #if" % (where, t))

    def eqexpr():
        v = primary()
        while peek() in ("==", "!="):
            op = take()
            r = primary()
            v = int(v == r) if op == "==" else int(v != r)
        return v

    def andexpr():
        v = eqexpr()
        while peek() == "&&":
            take()
            r = eqexpr()
            v = int(bool(v) and bool(r))
        return v

    def orexpr():
        v = andexpr()
        while peek() == "||":
            take()
            r = andexpr()
            v = int(bool(v) or bool(r))
        return v

    v = orexpr()
    if peek() is not None:
        raise ConvertError("%s: trailing tokens in #if %r" % (where, text))
    return v


def preprocess(src, fname):
    """Returns (code_text, includes).  Inactive lines and directives are
    blanked (newlines kept)."""
    text = strip_comments(src, fname)
    if re.search(r"\\\n", text):
        raise ConvertError("%s: line continuations are not supported" % fname)
    defines = {}
    includes = []
    stack = []  # (parent_active, this_branch_active, any_branch_taken)
    active = True
    out = []
    for lineno, line in enumerate(text.split("\n"), 1):
        where = "%s:%d" % (fname, lineno)
        s = line.strip()
        if s.startswith("#"):
            m = re.match(r"#\s*(\w+)\s*(.*)$", s)
            if not m:
                raise ConvertError("%s: bad directive %r" % (where, s))
            d, rest = m.group(1), m.group(2).strip()
            if d in ("ifdef", "ifndef"):
                if not re.fullmatch(r"[A-Za-z_]\w*", rest):
                    raise ConvertError("%s: bad #%s" % (where, d))
                isdef = rest in defines or rest in PREDEFINED
                cond = isdef if d == "ifdef" else not isdef
                stack.append((active, cond, cond))
                active = active and cond
            elif d == "if":
                cond = bool(eval_pp_expr(rest, defines, where)) if active else False
                stack.append((active, cond, cond))
                active = active and cond
            elif d == "elif":
                if not stack:
                    raise ConvertError("%s: #elif without #if" % where)
                parent, _, taken = stack.pop()
                cond = (not taken) and parent and bool(eval_pp_expr(rest, defines, where))
                stack.append((parent, cond, taken or cond))
                active = parent and cond
            elif d == "else":
                if not stack or rest:
                    raise ConvertError("%s: bad #else" % where)
                parent, _, taken = stack.pop()
                stack.append((parent, not taken, True))
                active = parent and not taken
            elif d == "endif":
                if not stack:
                    raise ConvertError("%s: #endif without #if" % where)
                parent, _, _ = stack.pop()
                active = parent
            elif not active:
                pass
            elif d == "include":
                includes.append(rest)
            elif d == "define":
                m2 = re.fullmatch(r"([A-Za-z_]\w*)(?:\s+(.*))?", rest)
                if not m2 or "(" in m2.group(1):
                    raise ConvertError("%s: unsupported #define %r" % (where, rest))
                defines[m2.group(1)] = (m2.group(2) or "").strip()
            elif d == "undef":
                defines.pop(rest, None)
            else:
                raise ConvertError("%s: unsupported directive #%s" % (where, d))
            out.append("")
        else:
            out.append(line if active else "")
    if stack:
        raise ConvertError("%s: unterminated #if" % fname)
    return "\n".join(out), includes, defines


# ---------------------------------------------------------------------------
# Tokenizer
# ---------------------------------------------------------------------------

TOKEN_RE = re.compile(r"""
    (?P<ws>\s+)
  | (?P<num>(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?[A-Za-z_]*)
  | (?P<id>[A-Za-z_]\w*)
  | (?P<str>"(?:[^"\\\n]|\\.)*")
  | (?P<chr>'(?:[^'\\\n]|\\.)*')
  | (?P<op>::|->|\+\+|--|\+=|-=|\*=|/=|==|!=|<=|>=|&&|\|\||<<|>>|[-+*/%<>=!&|^~?:;,.(){}\[\]])
""", re.X)


class Tok:
    __slots__ = ("kind", "text", "line")

    def __init__(self, kind, text, line):
        self.kind, self.text, self.line = kind, text, line

    def __repr__(self):
        return "%s:%r@%d" % (self.kind, self.text, self.line)


def tokenize(code, fname):
    toks = []
    pos = 0
    line = 1
    while pos < len(code):
        m = TOKEN_RE.match(code, pos)
        if not m:
            raise ConvertError("%s:%d: cannot tokenize %r" % (fname, line, code[pos:pos + 20]))
        kind = m.lastgroup
        text = m.group(kind)
        if kind != "ws":
            if kind == "num" and re.search(r"[A-Za-z_]$", text) and not re.fullmatch(
                    r"(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)", text):
                raise ConvertError("%s:%d: numeric literal with suffix %r not supported" % (fname, line, text))
            toks.append(Tok(kind, text, line))
        line += text.count("\n")
        pos = m.end()
    toks.append(Tok("eof", "", line))
    return toks


# ---------------------------------------------------------------------------
# Parser (AST = tuples)
# ---------------------------------------------------------------------------

# Known type names (without namespace qualification) -> op type.
VALUE_TYPES = {
    "Polygon": "poly", "PolyLine": "poly", "Rectangle": "poly", "Point": "point",
    "ConnEnd": "connend", "Checkpoint": "checkpoint",
    "HyperedgeNewAndDeletedObjectLists": "ndlists", "ConnEndList": "connendlist",
    "ConnRefList": "connlist", "JunctionRefList": "junctionlist",
    "ConnRefList::const_iterator": "citer", "JunctionRefList::const_iterator": "jiter",
    "ConnRefList::iterator": "citer", "JunctionRefList::iterator": "jiter",
    "vector<Checkpoint>": "checkpoints", "vector<Point>": "points",
    "pair<JunctionRef*,ConnRef*>": "jcpair", "pair<ConnEnd,ConnEnd>": "endpair",
    "int": "int", "bool": "bool", "double": "double", "unsigned int": "int",
    "unsigned": "int", "size_t": "int",
    "ostringstream": "ostream", "string": "string",
}
POINTER_TYPES = {
    "Router": "router", "ConnRef": "conn", "ShapeRef": "shape", "JunctionRef": "junction",
    "ShapeConnectionPin": "pin", "HyperedgeRerouter": "rerouter", "ClusterRef": "cluster",
    "void": "void",
}
NAMESPACES = ("Avoid", "std", "cola")
TYPE_BASES = set(k.split("<")[0].split("::")[0] for k in VALUE_TYPES) | set(POINTER_TYPES) | {
    "vector", "pair", "unsigned", "const", "static", "char"}


class Parser:
    def __init__(self, toks, fname):
        self.t = toks
        self.i = 0
        self.fname = fname

    def err(self, msg, tok=None):
        tok = tok or self.peek()
        raise ConvertError("%s:%d: %s (at %r)" % (self.fname, tok.line, msg, tok.text))

    def peek(self, k=0):
        return self.t[min(self.i + k, len(self.t) - 1)]

    def at(self, text, k=0):
        tok = self.peek(k)
        return tok.kind in ("op", "id") and tok.text == text

    def take(self, text=None):
        tok = self.peek()
        if text is not None and not self.at(text):
            self.err("expected %r" % text)
        self.i += 1
        return tok

    # --- names / types ---
    def qualified_name(self):
        """id(::id)* -> list of parts."""
        tok = self.peek()
        if tok.kind != "id":
            self.err("expected identifier")
        parts = [self.take().text]
        while self.at("::") and self.peek(1).kind == "id":
            self.take()
            parts.append(self.take().text)
        return parts

    def strip_ns(self, parts):
        while len(parts) > 1 and parts[0] in NAMESPACES:
            parts = parts[1:]
        return parts

    def try_type(self):
        """Parses a type at the current position if one starts here.  Returns
        (type_key, is_pointer, is_ref, is_const, is_static) or None (position
        unchanged)."""
        save = self.i
        is_static = is_const = False
        while self.at("static") or self.at("const"):
            if self.take().text == "static":
                is_static = True
            else:
                is_const = True
        if self.peek().kind != "id":
            self.i = save
            return None
        parts = self.strip_ns(self.qualified_name())
        if parts[0] == "unsigned":
            if self.at("int"):
                self.take()
                parts = ["unsigned int"]
        key = "::".join(parts)
        if key not in VALUE_TYPES and key not in POINTER_TYPES and key not in ("vector", "pair", "char"):
            self.i = save
            return None
        if key in ("vector", "pair"):
            self.take("<")
            args = []
            while True:
                sub = self.try_type()
                if sub is None:
                    self.err("unsupported template argument")
                args.append(sub[0] + ("*" if sub[1] else ""))
                if self.at(","):
                    self.take()
                    continue
                self.take(">")
                break
            key = "%s<%s>" % (key, ",".join(args))
            if self.at("::"):
                self.err("unsupported nested template type")
        is_ptr = is_ref = False
        while self.at("*") or self.at("&") or self.at("const"):
            t = self.take().text
            if t == "*":
                if is_ptr:
                    self.err("pointer to pointer not supported")
                is_ptr = True
            elif t == "&":
                is_ref = True
            else:
                is_const = True
        return (key, is_ptr, is_ref, is_const, is_static)

    # --- top level ---
    def parse_unit(self):
        items = []
        while self.peek().kind != "eof":
            if self.at(";"):
                self.take()
                continue
            if self.at("using"):
                self.take()
                if self.at("namespace"):
                    self.take()
                    ns = self.qualified_name()
                    items.append(("using_ns", "::".join(ns), self.peek().line))
                else:
                    items.append(("using", self.qualified_name(), self.peek().line))
                self.take(";")
                continue
            line = self.peek().line
            linkage = None
            if self.at("extern"):
                self.take()
                if self.peek().kind != "str":
                    self.err("expected linkage string")
                linkage = self.take().text
            is_static = False
            if self.at("static"):
                self.take()
                is_static = True
            rtype = self.try_type()
            if rtype is None:
                self.err("expected function definition")
            name = self.take().text
            self.take("(")
            params = []
            depth = 1
            cur = []
            while depth:
                tok = self.take()
                if tok.kind == "eof":
                    self.err("unterminated parameter list")
                if tok.text == "(":
                    depth += 1
                elif tok.text == ")":
                    depth -= 1
                    if depth == 0:
                        break
                if tok.text == "," and depth == 1:
                    params.append(cur)
                    cur = []
                else:
                    cur.append(tok)
            if cur:
                params.append(cur)
            body_start = self.i
            body = self.block()
            body_toks = self.t[body_start:self.i]
            items.append(("func", name, rtype, params, body, body_toks, line, linkage, is_static))
        return items

    # --- statements ---
    def block(self):
        self.take("{")
        stmts = []
        while not self.at("}"):
            if self.peek().kind == "eof":
                self.err("unterminated block")
            stmts.append(self.statement())
        self.take("}")
        return ("block", stmts)

    def statement(self):
        tok = self.peek()
        line = tok.line
        if self.at("{"):
            return self.block()
        if self.at(";"):
            self.take()
            return ("empty", line)
        if self.at("if"):
            self.take()
            self.take("(")
            cond = self.expr()
            self.take(")")
            then = self.statement()
            els = None
            if self.at("else"):
                self.take()
                els = self.statement()
            return ("if", cond, then, els, line)
        if self.at("for") or self.at("while") or self.at("do") or self.at("switch") or self.at("goto"):
            # Parsed only to be rejected (or matched as a known callback) later.
            kw = self.take().text
            if kw != "for":
                self.err("unsupported statement %r" % kw)
            self.take("(")
            depth = 1
            while depth:
                t = self.take()
                if t.text == "(":
                    depth += 1
                elif t.text == ")":
                    depth -= 1
            body = self.statement()
            return ("for", body, line)
        if self.at("return"):
            self.take()
            if self.at(";"):
                self.take()
                return ("return", None, line)
            e = self.expr()
            self.take(";")
            return ("return", e, line)
        if self.at("delete"):
            self.take()
            e = self.expr()
            self.take(";")
            return ("delete", e, line)
        typ = self.try_type()
        if typ is not None:
            if self.peek().kind == "id":
                name = self.take().text
                if self.at("("):
                    self.take()
                    args = self.args(")")
                    self.take(";")
                    return ("decl", typ, name, "ctor", args, line)
                if self.at("="):
                    self.take()
                    init = self.expr()
                    self.take(";")
                    return ("decl", typ, name, "copy", init, line)
                if self.at("["):
                    self.err("arrays not supported")
                self.take(";")
                return ("decl", typ, name, "default", None, line)
            # A type not followed by a declarator: functional cast expression
            # such as `Avoid::Point(1, 2).x` -- not used by any fixture.
            self.err("unsupported statement starting with a type")
        e = self.expr()
        self.take(";")
        return ("expr", e, line)

    def args(self, close):
        out = []
        if self.at(close):
            self.take()
            return out
        while True:
            out.append(self.assign_expr())
            if self.at(","):
                self.take()
                continue
            self.take(close)
            return out

    # --- expressions ---
    def expr(self):
        e = self.assign_expr()
        if self.at(","):
            self.err("comma operator not supported")
        return e

    def assign_expr(self):
        lhs = self.cond_expr()
        for op in ("=", "+=", "-=", "*=", "/="):
            if self.at(op):
                self.take()
                rhs = self.assign_expr()
                return ("assign", op, lhs, rhs)
        return lhs

    def cond_expr(self):
        c = self.binary(0)
        if self.at("?"):
            self.take()
            a = self.assign_expr()
            self.take(":")
            b = self.assign_expr()
            return ("cond", c, a, b)
        return c

    LEVELS = [["||"], ["&&"], ["|"], ["^"], ["&"], ["==", "!="], ["<", ">", "<=", ">="],
              ["<<", ">>"], ["+", "-"], ["*", "/", "%"]]

    def binary(self, level):
        if level == len(self.LEVELS):
            return self.unary()
        e = self.binary(level + 1)
        while self.peek().kind == "op" and self.peek().text in self.LEVELS[level]:
            op = self.take().text
            r = self.binary(level + 1)
            e = ("bin", op, e, r)
        return e

    def unary(self):
        if self.peek().kind == "op" and self.peek().text in ("!", "-", "+", "*", "&", "~"):
            op = self.take().text
            return ("unary", op, self.unary())
        if self.at("++") or self.at("--"):
            op = self.take().text
            return ("preinc", op, self.unary())
        if self.at("new"):
            self.take()
            parts = self.strip_ns(self.qualified_name())
            self.take("(")
            return ("new", "::".join(parts), self.args(")"))
        if self.at("("):
            # cast?
            save = self.i
            self.take()
            if self.peek().kind == "id":
                p2 = self.i
                parts = self.strip_ns(self.qualified_name())
                if len(parts) == 1 and parts[0] in ENUM_CASTS and self.at(")"):
                    self.take()
                    return ("cast", (parts[0], False, False, False, False), self.unary())
                self.i = p2
            typ = self.try_type()
            if typ is not None and self.at(")"):
                self.take()
                return ("cast", typ, self.unary())
            self.i = save
        return self.postfix()

    def postfix(self):
        e = self.primary()
        while True:
            if self.at("("):
                self.take()
                e = ("call", e, self.args(")"))
            elif self.at("["):
                self.take()
                idx = self.expr()
                self.take("]")
                e = ("index", e, idx)
            elif self.at(".") or self.at("->"):
                arrow = self.take().text == "->"
                if self.peek().kind != "id":
                    self.err("expected member name")
                e = ("member", e, self.take().text, arrow)
            elif self.at("++") or self.at("--"):
                e = ("postinc", self.take().text, e)
            else:
                return e

    def primary(self):
        tok = self.peek()
        if tok.kind == "num":
            self.take()
            return ("num", tok.text)
        if tok.kind == "str":
            self.take()
            s = tok.text
            while self.peek().kind == "str":  # adjacent literal concatenation
                s = s[:-1] + self.take().text[1:]
            return ("str", s)
        if tok.kind == "chr":
            self.err("character literals not supported")
        if self.at("("):
            self.take()
            e = self.expr()
            self.take(")")
            return ("paren", e)
        if tok.kind == "id":
            if tok.text in ("true", "false"):
                self.take()
                return ("bool", tok.text == "true")
            if tok.text in ("nullptr", "NULL"):
                self.take()
                return ("null",)
            parts = self.qualified_name()
            # template functional cast, e.g. std::vector<...>(...) -- unsupported
            return ("name", self.strip_ns(parts), parts)
        self.err("unexpected token in expression")


# ---------------------------------------------------------------------------
# Lowering
# ---------------------------------------------------------------------------

# Enumerators / constants -> (value, ctype)
ENUMS = {
    "PolyLineRouting": 1, "OrthogonalRouting": 2,
    "segmentPenalty": 0, "anglePenalty": 1, "crossingPenalty": 2, "clusterCrossingPenalty": 3,
    "fixedSharedPathPenalty": 4, "portDirectionPenalty": 5, "shapeBufferDistance": 6,
    "idealNudgingDistance": 7, "reverseDirectionPenalty": 8,
    "nudgeOrthogonalSegmentsConnectedToShapes": 0, "improveHyperedgeRoutesMovingJunctions": 1,
    "penaliseOrthogonalSharedPathsAtConnEnds": 2, "nudgeOrthogonalTouchingColinearSegments": 3,
    "performUnifyingNudgingPreprocessingStep": 4,
    "improveHyperedgeRoutesMovingAddingAndDeletingJunctions": 5,
    "nudgeSharedPathsWithCommonEndPoint": 6,
    "ConnType_None": 0, "ConnType_PolyLine": 1, "ConnType_Orthogonal": 2,
    "ConnDirNone": 0, "ConnDirUp": 1, "ConnDirDown": 2, "ConnDirLeft": 4, "ConnDirRight": 8,
    "ConnDirAll": 15,
    "TransformationType_CW90": 0, "TransformationType_CW180": 1, "TransformationType_CW270": 2,
    "TransformationType_FlipX": 3, "TransformationType_FlipY": 4,
    "CONNECTIONPIN_UNSET": 2147483647, "CONNECTIONPIN_CENTRE": 2147483646,
}
DOUBLE_CONSTS = {
    "ATTACH_POS_TOP": "0", "ATTACH_POS_CENTRE": "0.5", "ATTACH_POS_BOTTOM": "1",
    "ATTACH_POS_LEFT": "0", "ATTACH_POS_RIGHT": "1",
    "ATTACH_POS_MIN_OFFSET": "0", "ATTACH_POS_MAX_OFFSET": "-1",
}
ENUM_CASTS = {"ConnType", "ConnDirFlags", "ConnDirFlag", "PenaltyType", "RoutingParameter",
              "RoutingOption", "ShapeTransformationType", "RouterFlag"}

PTR_OP_TYPES = set(POINTER_TYPES.values())

# The two connector callbacks used by upstream fixtures, compared token by
# token (whitespace-insensitive) with the fixture's function body.
CALLBACK_TEMPLATE = """{
    Avoid::ConnRef *connRef = (Avoid::ConnRef *) ptr;
    printf("Connector %u needs rerouting!\\n", connRef->id());
    const Avoid::PolyLine& route = connRef->ROUTE();
    printf("New path: ");
    for (size_t i = 0; i < route.ps.size(); ++i)
    {
        printf("%s(%f, %f)", (i > 0) ? "-" : "",
                route.ps[i].x, route.ps[i].y);
    }
    printf("\\n");
}"""


def callback_kind(body_toks):
    text = " ".join(t.text for t in body_toks)
    for kind in ("route", "displayRoute"):
        tmpl = tokenize(CALLBACK_TEMPLATE.replace("ROUTE", kind), "<template>")[:-1]
        if text == " ".join(t.text for t in tmpl):
            return kind
    return None


def c_int_div(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def fmt_num(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    r = repr(float(v))
    if r in ("inf", "-inf", "nan"):
        raise ConvertError("non-finite constant")
    return r


class T:
    """A lowered expression: op-script code, static op type, compile-time
    value (or None), and for lvalues the op lvalue code."""
    __slots__ = ("code", "ty", "ct", "lv")

    def __init__(self, code, ty, ct=None, lv=None):
        self.code, self.ty, self.ct, self.lv = code, ty, ct, lv


NUMERIC = ("int", "double", "bool")


class Func:
    def __init__(self, item):
        (_, self.name, self.rtype, self.params, self.body, self.body_toks, self.line,
         self.linkage, self.is_static) = item
        self.statics = {}  # name -> T (compile time)
        self.param_names = []
        for p in self.params:
            texts = [t.text for t in p]
            if texts == ["void"]:
                continue
            ids = [t.text for t in p if t.kind == "id"]
            self.param_names.append(ids[-1] if ids else None)


class Lowerer:
    def __init__(self, fixture, fname, smoke):
        self.fixture = fixture
        self.fname = fname
        self.smoke = smoke
        self.out = []
        self.vars = {}       # flat runtime namespace: name -> op type
        self.scopes = []     # stack of dicts: name -> T alias / ct value
        self.funcs = {}
        self.callbacks = {}  # function name -> kind
        self.router_deletes = 0
        self.notes = []
        self.cond_depth = 0
        self.returned = False

    def err(self, line, msg):
        raise ConvertError("%s:%s: %s" % (self.fname, line, msg))

    def emit(self, line):
        if self.returned:
            raise ConvertError("%s: statement after return" % self.fname)
        self.out.append("    " * self.cond_depth + line)

    # --- names ---
    def lookup(self, name):
        for sc in reversed(self.scopes):
            if name in sc:
                return sc[name]
        return None

    def declare(self, line, name, ty):
        if self.lookup(name) is not None or name in self.vars:
            self.err(line, "redeclaration of %r (flattened scopes)" % name)
        if name in ("if", "else", "end", "let", "set", "do", "true", "false", "null") or \
                not re.fullmatch(r"[A-Za-z_]\w*", name):
            self.err(line, "unsupported variable name %r" % name)
        self.vars[name] = ty
        self.scopes[-1][name] = T(name, ty, None, name)

    # --- conversions ---
    def conv(self, line, e, target, what="value"):
        if e.ty == target:
            return e
        if target in ("double", "int", "bool") and e.ty in NUMERIC:
            if e.ct is not None:
                v = e.ct
                if target == "double":
                    return T(e.code if not isinstance(v, bool) else fmt_num(int(v)), "double", float(v))
                if target == "int":
                    if isinstance(v, float):
                        if v != int(v):
                            self.err(line, "implicit double->int truncation of %r" % v)
                        v = int(v)
                    return T(fmt_num(int(v)), "int", int(v))
                return T(fmt_num(bool(v)), "bool", bool(v))
            return T("(%s %s)" % (target, e.code), target)
        if target in PTR_OP_TYPES and e.ty == "nullptr":
            return T("null", target)
        if target == "connend" and e.ty == "point":
            return T("(ConnEndPt1 %s)" % e.code, "connend")
        if target == "checkpoint" and e.ty == "point":
            return T("(Checkpoint %s)" % e.code, "checkpoint")
        self.err(line, "cannot convert %s of type %s to %s" % (what, e.ty, target))

    def ct_str(self, line, e):
        if e.ty != "string" or e.ct is None:
            self.err(line, "expected compile-time string")
        return e.ct

    # --- expressions ---
    def lower(self, line, n):
        k = n[0]
        if k == "paren":
            return self.lower(line, n[1])
        if k == "num":
            text = n[1]
            if re.fullmatch(r"\d+", text):
                if len(text) > 1 and text.startswith("0"):
                    self.err(line, "octal literal %r" % text)
                return T(text, "int", int(text))
            return T(text, "double", float(text))
        if k == "bool":
            return T("true" if n[1] else "false", "bool", n[1])
        if k == "null":
            return T("null", "nullptr")
        if k == "str":
            raw = n[1][1:-1]
            return T(n[1], "string", decode_c_string(raw))
        if k == "name":
            parts = n[1]
            if len(parts) == 1:
                v = self.lookup(parts[0])
                if v is not None:
                    return v
            key = parts[-1] if len(parts) == 1 else None
            if key in ENUMS:
                return T(str(ENUMS[key]), "int", ENUMS[key])
            if key in DOUBLE_CONSTS:
                return T(DOUBLE_CONSTS[key], "double", float(DOUBLE_CONSTS[key]))
            self.err(line, "unknown name %s" % "::".join(n[2]))
        if k == "cast":
            typ = n[1]
            base = typ[0]
            if base in ENUM_CASTS and not typ[1]:
                e = self.lower(line, n[2])
                return self.conv(line, e, "int", "cast operand")
            self.err(line, "unsupported cast to %s" % base)
        if k == "unary":
            op = n[1]
            e = self.lower(line, n[2])
            if op == "-":
                if e.ty not in ("int", "double"):
                    self.err(line, "unary minus on %s" % e.ty)
                if e.ct is not None:
                    if e.code.startswith("-") or e.code.startswith("("):
                        return T(fmt_num(-e.ct), e.ty, -e.ct)
                    return T("-" + e.code, e.ty, -e.ct)
                return T("(neg %s)" % e.code, e.ty)
            if op == "+":
                return e
            if op == "!":
                e = self.conv(line, e, "bool") if e.ty in NUMERIC else e
                if e.ty != "bool":
                    self.err(line, "! on %s" % e.ty)
                if e.ct is not None:
                    return T(fmt_num(not e.ct), "bool", not e.ct)
                return T("(not %s)" % e.code, "bool")
            if op == "*":
                if e.ty == "jiter":
                    return T("(deref %s)" % e.code, "junction")
                if e.ty == "citer":
                    return T("(deref %s)" % e.code, "conn")
                self.err(line, "unsupported dereference of %s" % e.ty)
            self.err(line, "unsupported unary %s" % op)
        if k == "preinc":
            e = self.lower(line, n[2])
            if e.ty == "int" and e.lv and e.lv.startswith("ct:"):
                return self.ct_incdec(line, e, n[1], prefix=True)
            self.err(line, "unsupported ++/-- in expression")
        if k == "postinc":
            self.err(line, "unsupported postfix ++/-- in expression")
        if k == "bin":
            return self.lower_bin(line, n)
        if k == "cond":
            c = self.lower(line, n[1])
            c = self.conv(line, c, "bool") if c.ty in NUMERIC else c
            if c.ty != "bool":
                self.err(line, "condition of type %s" % c.ty)
            a = self.lower(line, n[2])
            b = self.lower(line, n[3])
            if a.ty in NUMERIC and b.ty in NUMERIC:
                ty = "double" if "double" in (a.ty, b.ty) else ("int" if "int" in (a.ty, b.ty) else "bool")
                a, b = self.conv(line, a, ty), self.conv(line, b, ty)
            elif a.ty != b.ty:
                self.err(line, "?: with %s and %s" % (a.ty, b.ty))
            else:
                ty = a.ty
            if c.ct is not None and a.ct is not None and b.ct is not None:
                v = a.ct if c.ct else b.ct
                return T(fmt_num(v), ty, v)
            return T("(? %s %s %s)" % (c.code, a.code, b.code), ty)
        if k == "new":
            return self.lower_ctor(line, n[1], n[2], heap=True)
        if k == "call":
            return self.lower_call(line, n)
        if k == "member":
            return self.lower_member(line, n)
        if k == "index":
            base = self.lower(line, n[1])
            idx = self.conv(line, self.lower(line, n[2]), "int", "index")
            if base.ty == "points_ps":  # poly.ps[i]
                lv = "(ps %s %s)" % (base.lv, idx.code) if base.lv else None
                return T("(ps %s %s)" % (base.code[len("(psvec "):-1], idx.code), "point", None, lv)
            if base.ty in ("checkpoints", "points"):
                lv = "(at %s %s)" % (base.lv, idx.code) if base.lv else None
                return T("(at %s %s)" % (base.code, idx.code),
                         "checkpoint" if base.ty == "checkpoints" else "point", None, lv)
            self.err(line, "unsupported indexing of %s" % base.ty)
        if k == "assign":
            self.err(line, "assignment used as a value")
        self.err(line, "unsupported expression %r" % (k,))

    def ct_incdec(self, line, e, op, prefix):
        name = e.lv[3:]
        cur = self.lookup(name)
        if self.cond_depth:
            self.err(line, "compile-time state mutated inside a runtime condition")
        nv = cur.ct + (1 if op == "++" else -1)
        self.set_ct(name, T(str(nv), "int", nv, cur.lv))
        return T(str(nv if prefix else cur.ct), "int", nv if prefix else cur.ct)

    def set_ct(self, name, val):
        for sc in reversed(self.scopes):
            if name in sc:
                sc[name] = val
                return
        raise AssertionError(name)

    def lower_bin(self, line, n):
        op = n[1]
        a = self.lower(line, n[2])
        if op == "<<" and a.ty == "ostream":
            b = self.lower(line, n[3])
            if b.ct is None:
                self.err(line, "ostream << runtime value")
            if self.cond_depth:
                self.err(line, "compile-time state mutated inside a runtime condition")
            s = b.ct if b.ty == "string" else fmt_num(b.ct)
            name = a.lv[3:]
            nv = T(a.code, "ostream", a.ct + s, a.lv)
            self.set_ct(name, nv)
            return nv
        b = self.lower(line, n[3])
        if op in ("&&", "||"):
            a = self.conv(line, a, "bool") if a.ty in NUMERIC else a
            b = self.conv(line, b, "bool") if b.ty in NUMERIC else b
            if a.ty != "bool" or b.ty != "bool":
                self.err(line, "%s on non-bool" % op)
            if a.ct is not None and b.ct is not None:
                v = (a.ct and b.ct) if op == "&&" else (a.ct or b.ct)
                return T(fmt_num(v), "bool", v)
            return T("(%s %s %s)" % ("and" if op == "&&" else "or", a.code, b.code), "bool")
        if op in ("==", "!="):
            if a.ty in NUMERIC and b.ty in NUMERIC:
                pass
            elif a.ty == b.ty and a.ty in PTR_OP_TYPES | {"jiter", "citer"}:
                return T("(%s %s %s)" % (op, a.code, b.code), "bool")
            elif a.ty in PTR_OP_TYPES and b.ty == "nullptr":
                return T("(%s %s null)" % (op, a.code), "bool")
            else:
                self.err(line, "%s between %s and %s" % (op, a.ty, b.ty))
        if a.ty not in NUMERIC or b.ty not in NUMERIC:
            self.err(line, "operator %s on %s and %s" % (op, a.ty, b.ty))
        if op in ("|", "&", "^", "%"):
            if a.ty != "int" or b.ty != "int":
                self.err(line, "integer operator %s on non-int" % op)
        # usual arithmetic conversions (bool promotes to int)
        ty = "double" if "double" in (a.ty, b.ty) else "int"
        a2 = self.conv(line, a, ty)
        b2 = self.conv(line, b, ty)
        res_ty = "bool" if op in ("==", "!=", "<", ">", "<=", ">=") else ty
        if a2.ct is not None and b2.ct is not None:
            x, y = a2.ct, b2.ct
            if op == "+":
                v = x + y
            elif op == "-":
                v = x - y
            elif op == "*":
                v = x * y
            elif op == "/":
                if y == 0:
                    self.err(line, "division by zero")
                v = c_int_div(x, y) if ty == "int" else x / y
            elif op == "%":
                v = x - c_int_div(x, y) * y
            elif op == "|":
                v = x | y
            elif op == "&":
                v = x & y
            elif op == "^":
                v = x ^ y
            elif op == "==":
                v = x == y
            elif op == "!=":
                v = x != y
            elif op == "<":
                v = x < y
            elif op == ">":
                v = x > y
            elif op == "<=":
                v = x <= y
            elif op == ">=":
                v = x >= y
            else:
                self.err(line, "unsupported operator %s" % op)
            if ty == "int" and res_ty == "int" and not (-2**31 <= v < 2**31):
                self.err(line, "integer overflow in constant expression")
            return T(fmt_num(v), res_ty, v)
        if op in ("<<", ">>", "^"):
            self.err(line, "unsupported runtime operator %s" % op)
        return T("(%s %s %s)" % (op, a2.code, b2.code), res_ty)

    def lower_args(self, line, args):
        return [self.lower(line, a) for a in args]

    def check_arity(self, line, what, args, lo, hi=None):
        hi = lo if hi is None else hi
        if not (lo <= len(args) <= hi):
            self.err(line, "%s: expected %d..%d arguments, got %d" % (what, lo, hi, len(args)))

    def lower_ctor(self, line, tname, raw_args, heap):
        args = self.lower_args(line, raw_args)
        c = lambda e, t: self.conv(line, e, t, "argument of %s" % tname)
        tname = tname.split("::")[-1] if tname.split("::")[0] in NAMESPACES else tname
        if heap:
            if tname == "Router":
                self.check_arity(line, tname, args, 1)
                return T("(Router %s)" % c(args[0], "int").code, "router")
            if tname == "ShapeRef":
                self.check_arity(line, tname, args, 2, 3)
                idv = c(args[2], "int").code if len(args) > 2 else "0"
                return T("(ShapeRef %s %s %s)" % (c(args[0], "router").code, c(args[1], "poly").code, idv), "shape")
            if tname == "JunctionRef":
                self.check_arity(line, tname, args, 2, 3)
                idv = c(args[2], "int").code if len(args) > 2 else "0"
                return T("(JunctionRef %s %s %s)" % (c(args[0], "router").code, c(args[1], "point").code, idv),
                         "junction")
            if tname == "ConnRef":
                self.check_arity(line, tname, args, 1, 4)
                r = c(args[0], "router").code
                if len(args) <= 2:
                    idv = c(args[1], "int").code if len(args) > 1 else "0"
                    return T("(ConnRef %s %s)" % (r, idv), "conn")
                if len(args) >= 3:
                    idv = c(args[3], "int").code if len(args) > 3 else "0"
                    return T("(ConnRefEnds %s %s %s %s)" % (r, c(args[1], "connend").code,
                                                            c(args[2], "connend").code, idv), "conn")
            if tname == "ShapeConnectionPin":
                if args and args[0].ty == "junction":
                    self.check_arity(line, tname, args, 2, 3)
                    d = c(args[2], "int").code if len(args) > 2 else "0"
                    return T("(PinJunction %s %s %s)" % (args[0].code, c(args[1], "int").code, d), "pin")
                if len(args) == 7:
                    return T("(Pin %s %s %s %s %s %s %s)" % (
                        c(args[0], "shape").code, c(args[1], "int").code, c(args[2], "double").code,
                        c(args[3], "double").code, c(args[4], "bool").code, c(args[5], "double").code,
                        c(args[6], "int").code), "pin")
                if len(args) == 6:
                    return T("(PinOld %s %s %s %s %s %s)" % (
                        c(args[0], "shape").code, c(args[1], "int").code, c(args[2], "double").code,
                        c(args[3], "double").code, c(args[4], "double").code, c(args[5], "int").code), "pin")
                self.err(line, "ShapeConnectionPin with %d arguments" % len(args))
            self.err(line, "unsupported new %s" % tname)
        # value construction
        if tname == "Point":
            self.check_arity(line, tname, args, 2)
            return T("(Point %s %s)" % (c(args[0], "double").code, c(args[1], "double").code), "point")
        if tname in ("Polygon", "PolyLine"):
            self.check_arity(line, tname, args, 0, 1)
            if not args:
                return T("(Polygon 0)", "poly")
            return T("(Polygon %s)" % c(args[0], "int").code, "poly")
        if tname == "Rectangle":
            if len(args) == 2:
                return T("(Rectangle %s %s)" % (c(args[0], "point").code, c(args[1], "point").code), "poly")
            self.err(line, "unsupported Rectangle constructor")
        if tname == "ConnEnd":
            if not args:
                return T("(ConnEnd)", "connend")
            if args[0].ty == "point":
                if len(args) == 1:
                    return T("(ConnEndPt1 %s)" % args[0].code, "connend")
                self.check_arity(line, tname, args, 2)
                return T("(ConnEndPt %s %s)" % (args[0].code, c(args[1], "int").code), "connend")
            if args[0].ty == "shape":
                self.check_arity(line, tname, args, 2)
                return T("(ConnEndShape %s %s)" % (args[0].code, c(args[1], "int").code), "connend")
            if args[0].ty == "junction":
                self.check_arity(line, tname, args, 1)
                return T("(ConnEndJunction %s)" % args[0].code, "connend")
            self.err(line, "unsupported ConnEnd constructor from %s" % args[0].ty)
        if tname == "Checkpoint":
            if len(args) == 1:
                return T("(Checkpoint %s)" % c(args[0], "point").code, "checkpoint")
            self.check_arity(line, tname, args, 3)
            return T("(CheckpointDirs %s %s %s)" % (c(args[0], "point").code, c(args[1], "int").code,
                                                    c(args[2], "int").code), "checkpoint")
        self.err(line, "unsupported construction of %s" % tname)

    def lower_member(self, line, n):
        _, obj_n, name, arrow = n
        obj = self.lower(line, obj_n)
        is_ptr = obj.ty in PTR_OP_TYPES
        if arrow != is_ptr:
            self.err(line, "%s used on %s" % ("->" if arrow else ".", obj.ty))
        if obj.ty == "poly":
            if name == "ps":
                return T("(psvec %s)" % obj.code, "points_ps", None, obj.lv)
            if name == "_id":
                return T("(_id %s)" % obj.code, "int", None, "(_id %s)" % obj.lv if obj.lv else None)
        if obj.ty == "point" and name in ("x", "y"):
            lv = "(%s %s)" % (name, obj.lv) if obj.lv and not obj.lv.startswith("(") else None
            return T("(%s %s)" % (name, obj.code), "double", None, lv)
        if obj.ty in ("jcpair", "endpair") and name in ("first", "second"):
            ty = {"jcpair": ("junction", "conn"), "endpair": ("connend", "connend")}[obj.ty]
            return T("(%s %s)" % (name, obj.code), ty[0] if name == "first" else ty[1])
        if obj.ty == "ndlists" and name in ("newJunctionList", "deletedJunctionList"):
            return T("(%s %s)" % (name, obj.code), "junctionlist")
        if obj.ty == "ndlists" and name in ("newConnectorList", "deletedConnectorList",
                                            "changedConnectorList"):
            return T("(%s %s)" % (name, obj.code), "connlist")
        self.err(line, "unsupported member %s of %s" % (name, obj.ty))

    def lower_call(self, line, n):
        _, callee, raw_args = n
        if callee[0] == "name":
            parts = callee[1]
            fname = parts[-1]
            if len(parts) == 1 and fname == "fabs":
                args = self.lower_args(line, raw_args)
                self.check_arity(line, "fabs", args, 1)
                return T("(fabs %s)" % self.conv(line, args[0], "double").code, "double")
            if len(parts) == 1 and fname in ("Point", "Polygon", "PolyLine", "Rectangle", "ConnEnd",
                                             "Checkpoint"):
                return self.lower_ctor(line, fname, raw_args, heap=False)
            self.err(line, "unsupported call of %s" % "::".join(callee[2]))
        if callee[0] != "member":
            self.err(line, "unsupported call")
        _, obj_n, meth, arrow = callee
        # poly.ps.resize(n)
        if obj_n[0] == "member" and obj_n[2] == "ps":
            poly = self.lower(line, obj_n[1])
            if poly.ty == "poly" and meth in ("resize", "size"):
                args = self.lower_args(line, raw_args)
                if meth == "size":
                    self.check_arity(line, meth, args, 0)
                    return T("(size %s)" % poly.code, "int")
                self.check_arity(line, meth, args, 1)
                if not poly.lv:
                    self.err(line, "resize of a temporary")
                return T("(resize %s %s)" % (poly.lv, self.conv(line, args[0], "int").code), "void")
            self.err(line, "unsupported ps.%s" % meth)
        obj = self.lower(line, obj_n)
        is_ptr = obj.ty in PTR_OP_TYPES
        if arrow != is_ptr:
            self.err(line, "%s used on %s" % ("->" if arrow else ".", obj.ty))
        args = self.lower_args(line, raw_args)
        sig = METHODS.get((obj.ty, meth))
        if obj.ty == "ostream" and meth == "str":
            self.check_arity(line, meth, args, 0)
            return T('"%s"' % obj.ct, "string", obj.ct)
        if sig is None:
            if self.smoke and (obj.ty, meth) in MODERNISE:
                return MODERNISE[(obj.ty, meth)](self, line, obj, args)
            self.err(line, "unsupported method %s::%s" % (obj.ty, meth))
        return sig(self, line, obj, args)

    # --- statements ---
    def stmt(self, s, ctx):
        k = s[0]
        line = s[-1]
        if k == "block":
            for x in s[1]:
                self.stmt(x, ctx)
            return
        if k == "empty":
            return
        if k == "decl":
            return self.decl(s)
        if k == "if":
            _, cond, then, els, line = s
            c = self.lower(line, cond)
            c = self.conv(line, c, "bool") if c.ty in NUMERIC else c
            if c.ty != "bool":
                self.err(line, "if condition of type %s" % c.ty)
            self.emit("if %s" % c.code)
            self.cond_depth += 1
            self.scopes.append({})
            self.stmt(then, ctx)
            self.scopes.pop()
            self.cond_depth -= 1
            if els is not None:
                self.emit("else")
                self.cond_depth += 1
                self.scopes.append({})
                self.stmt(els, ctx)
                self.scopes.pop()
                self.cond_depth -= 1
            self.emit("end")
            return
        if k == "for":
            self.err(line, "loops are not supported")
        if k == "return":
            return self.ret(s, ctx)
        if k == "delete":
            e = self.lower(line, s[1])
            if e.ty != "router" or e.lv is None:
                self.err(line, "delete of %s not supported" % e.ty)
            if self.cond_depth:
                self.err(line, "delete inside a condition")
            self.router_deletes += 1
            if self.smoke:
                g = "-"
            else:
                g = "%s-final" % self.fixture + ("" if self.router_deletes == 1 else str(self.router_deletes))
            self.emit("delete %s %s" % (e.code, g))
            return
        if k == "expr":
            return self.expr_stmt(line, s[1])
        self.err(line, "unsupported statement %s" % k)

    def ret(self, s, ctx):
        _, e, line = s
        if ctx["func"] is not None and ctx["func"].name != "main":
            # Inlined function: only a trailing return is supported.
            if not ctx["last"] or self.cond_depth:
                self.err(line, "return in the middle of an inlined function")
            if e is None:
                return
            v = self.lower(line, e)
            if ctx["call_ctx"] == "return":
                self.emit("ret %s" % self.conv(line, v, "int", "return value").code)
                self.returned = True
            else:
                self.emit("do %s" % v.code)
            return
        if self.cond_depth:
            self.err(line, "return inside a condition")
        if e is None:
            self.err(line, "return without value in main")
        if e[0] == "call" and e[1][0] == "name" and len(e[1][1]) == 1 and e[1][1][0] in self.funcs:
            self.inline(line, e[1][1][0], e[2], "return")
            if not self.returned:
                self.err(line, "inlined function did not return a value")
            return
        v = self.lower(line, e)
        self.emit("ret %s" % self.conv(line, v, "int", "return value").code)
        self.returned = True

    def decl(self, s):
        _, typ, name, form, init, line = s
        key, is_ptr, is_ref, is_const, is_static = typ
        if is_ref:
            self.err(line, "reference declarations not supported")
        if is_ptr:
            ty = POINTER_TYPES.get(key)
        else:
            ty = VALUE_TYPES.get(key)
        if ty is None or ty == "void":
            self.err(line, "unsupported declaration type %s%s" % (key, "*" if is_ptr else ""))
        # compile-time entities
        if ty in ("ostream", "string"):
            if form != "default" or ty == "string":
                self.err(line, "unsupported %s declaration" % key)
            self.scopes[-1][name] = T(name, "ostream", "", "ct:" + name)
            return
        if is_static:
            func = self.cur_func
            if ty != "int" or form != "copy":
                self.err(line, "unsupported static declaration")
            if name not in func.statics:
                v = self.lower(line, init)
                if v.ct is None:
                    self.err(line, "static initialiser is not constant")
                func.statics[name] = T(str(v.ct), "int", int(v.ct), "ct:" + name)
            self.scopes[-1][name] = func.statics[name]
            self.static_scope_names.append(name)
            return
        if is_const and ty in NUMERIC and form == "copy":
            v = self.conv(line, self.lower(line, init), ty)
            if v.ct is None:
                self.err(line, "const initialiser is not constant")
            self.scopes[-1][name] = T(v.code, ty, v.ct)
            return
        if ty in ("points_ps",):
            self.err(line, "bad type")
        if form == "default":
            if ty in PTR_OP_TYPES or ty in NUMERIC:
                self.err(line, "uninitialised %s declaration" % ty)
            if ty == "points":
                self.err(line, "std::vector<Point> is not supported")
            self.declare(line, name, ty)
            self.emit("decl %s %s" % (name, ty))
            return
        if form == "ctor":
            args = self.lower_args(line, init)
            if ty == "poly" and key == "Rectangle":
                v = self.lower_ctor(line, "Rectangle", init, heap=False)
            elif ty == "poly":
                self.check_arity(line, key, args, 1)
                v = T("(Polygon %s)" % self.conv(line, args[0], "int").code, "poly")
            elif ty == "point":
                v = self.lower_ctor(line, "Point", init, heap=False)
            elif ty == "connend":
                v = self.lower_ctor(line, "ConnEnd", init, heap=False)
            elif ty == "checkpoints":
                self.check_arity(line, key, args, 1)
                v = T("(Checkpoints %s)" % self.conv(line, args[0], "int").code, "checkpoints")
            elif ty in NUMERIC:
                self.check_arity(line, key, args, 1)
                v = self.conv(line, args[0], ty)
            else:
                self.err(line, "unsupported direct-initialisation of %s" % ty)
        else:
            raw = init
            v = self.lower(line, raw)
            if ty == "poly" and v.ty == "poly":
                pass
            v = self.conv(line, v, ty, "initialiser of %s" % name)
        if ty == "points":
            self.err(line, "std::vector<Point> is not supported")
        self.declare(line, name, ty)
        self.emit("let %s %s %s" % (name, ty, v.code))

    def expr_stmt(self, line, e):
        k = e[0]
        if k == "assign":
            _, op, lhs_n, rhs_n = e
            lhs = self.lower(line, lhs_n)
            rhs = self.lower(line, rhs_n)
            if lhs.lv is None or lhs.lv.startswith("ct:"):
                self.err(line, "unsupported assignment target")
            if op != "=":
                if lhs.ty not in ("int", "double"):
                    self.err(line, "compound assignment on %s" % lhs.ty)
                rhs = self.lower_bin(line, ("bin", op[0], lhs_n, rhs_n))
            rhs = self.conv(line, rhs, lhs.ty, "assigned value")
            self.emit("set %s %s" % (lhs.lv, rhs.code))
            return
        if k == "postinc" or k == "preinc":
            op, target = (e[1], e[2])
            t = self.lower(line, target)
            if t.ty in ("jiter", "citer") and t.lv and not t.lv.startswith("ct:"):
                if op != "++":
                    self.err(line, "iterator decrement")
                self.emit("set %s (next %s)" % (t.lv, t.code))
                return
            if t.ty == "int" and t.lv and t.lv.startswith("ct:"):
                self.ct_incdec(line, t, op, prefix=True)
                return
            self.err(line, "unsupported increment")
        if k == "bin" and e[1] == "<<":
            v = self.lower(line, e)
            if v.ty != "ostream":
                self.err(line, "unsupported << statement")
            return
        if k == "new":
            v = self.lower(line, e)
            self.emit("do %s" % v.code)
            return
        if k == "call":
            callee = e[1]
            if callee[0] == "name" and len(callee[1]) == 1:
                fname = callee[1][0]
                if fname == "printf":
                    args = self.lower_args(line, e[2])
                    if len(args) != 1 or args[0].ty != "string":
                        self.err(line, "printf with format arguments not supported")
                    if "%" in args[0].ct.replace("%%", ""):
                        self.err(line, "printf with conversions not supported")
                    self.emit("print %s" % args[0].code)
                    return
                if fname in ("assert", "COLA_ASSERT"):
                    args = self.lower_args(line, e[2])
                    self.check_arity(line, fname, args, 1)
                    c = self.conv(line, args[0], "bool") if args[0].ty in NUMERIC else args[0]
                    if c.ty != "bool":
                        self.err(line, "assert on %s" % c.ty)
                    self.emit("assert %s" % c.code)
                    return
                if fname in self.funcs:
                    self.inline(line, fname, e[2], "stmt")
                    return
            if callee[0] == "member":
                _, obj_n, meth, arrow = callee
                if meth == "outputDiagram":
                    r = self.lower(line, obj_n)
                    if r.ty != "router":
                        self.err(line, "outputDiagram on %s" % r.ty)
                    args = self.lower_args(line, e[2])
                    self.check_arity(line, meth, args, 1)
                    path = self.ct_str(line, args[0])
                    self.emit("output %s %s" % (r.code, self.golden_name(line, path)))
                    return
                if meth == "setCallback":
                    c = self.lower(line, obj_n)
                    if c.ty != "conn" or len(e[2]) != 2:
                        self.err(line, "unsupported setCallback")
                    fn = e[2][0]
                    if fn[0] != "name" or len(fn[1]) != 1 or fn[1][0] not in self.callbacks:
                        self.err(line, "setCallback with an unknown callback function")
                    arg = self.lower(line, e[2][1])
                    if arg.code != c.code:
                        self.err(line, "setCallback context pointer is not the connector itself")
                    self.emit("callback %s %s" % (c.code, self.callbacks[fn[1][0]]))
                    return
            v = self.lower(line, e)
            if v.ct is not None and v.ty in NUMERIC:
                return  # discarded constant
            self.emit("do %s" % v.code)
            return
        self.err(line, "unsupported expression statement")

    def golden_name(self, line, path):
        if path.startswith("output/"):
            g = path[len("output/"):]
        else:
            g = "%s-%s" % (self.fixture, path)
        if not re.fullmatch(r"[\w.-]+", g):
            self.err(line, "unsupported output name %r" % path)
        return g

    def inline(self, line, fname, raw_args, call_ctx):
        func = self.funcs[fname]
        args = self.lower_args(line, raw_args)
        if len(args) != len(func.param_names):
            self.err(line, "call of %s with %d arguments" % (fname, len(args)))
        if any(a.ty in NUMERIC and a.ct is None for a in args):
            self.err(line, "runtime scalar argument to inlined function")
        if self.cond_depth:
            self.err(line, "inlined call inside a condition")
        scope = {}
        for p, a in zip(func.param_names, args):
            scope[p] = a
        self.scopes.append(scope)
        prev_func = self.cur_func
        prev_static = self.static_scope_names
        self.cur_func = func
        self.static_scope_names = []
        stmts = func.body[1]
        for i, s in enumerate(stmts):
            self.stmt(s, {"func": func, "call_ctx": call_ctx, "last": i == len(stmts) - 1})
        # persist compile-time statics
        for nm in self.static_scope_names:
            func.statics[nm] = scope[nm]
        self.cur_func = prev_func
        self.static_scope_names = prev_static
        # runtime locals of the inlined function stay in the flat namespace
        for nm, v in scope.items():
            if nm not in func.param_names and nm in self.vars:
                self.scopes[-2][nm] = v
        self.scopes.pop()

    def run(self, items):
        main = None
        for it in items:
            if it[0] == "func":
                f = Func(it)
                if f.name in self.funcs:
                    self.err(f.line, "function %s defined twice" % f.name)
                kind = callback_kind(f.body_toks)
                if kind is not None:
                    self.callbacks[f.name] = kind
                    continue
                if f.name == "main":
                    main = f
                else:
                    self.funcs[f.name] = f
            elif it[0] == "using_ns":
                if it[1] not in ("Avoid", "cola"):
                    self.err(it[2], "using namespace %s" % it[1])
            elif it[0] == "using":
                if it[1][0] != "Avoid" or len(it[1]) != 2 or it[1][1] not in ENUMS:
                    self.err(it[2], "unsupported using-declaration")
        if main is None:
            raise ConvertError("%s: no main()" % self.fname)
        self.scopes = [{}]
        self.cur_func = main
        self.static_scope_names = []
        stmts = main.body[1]
        for i, s in enumerate(stmts):
            if self.returned:
                if s[0] == "empty":
                    continue
                self.err(s[-1], "statement after return")
            self.stmt(s, {"func": main, "call_ctx": None, "last": i == len(stmts) - 1})
        if not self.returned:
            # falling off the end of main returns 0
            self.emit("ret 0")
            self.returned = True


def decode_c_string(raw):
    out = []
    i = 0
    esc = {"n": "\n", "t": "\t", "\\": "\\", '"': '"', "'": "'", "0": "\0", "r": "\r"}
    while i < len(raw):
        ch = raw[i]
        if ch == "\\":
            nxt = raw[i + 1]
            if nxt not in esc:
                raise ConvertError("unsupported string escape \\%s" % nxt)
            out.append(esc[nxt])
            i += 2
        else:
            out.append(ch)
            i += 1
    return "".join(out)


# --- method table -----------------------------------------------------------

def _m(name, sig, ret, defaults=()):
    """Builds a method lowering: `sig` lists the op types of the arguments
    (after the receiver); trailing `defaults` (op code strings) are filled in
    when omitted."""
    def f(lw, line, obj, args):
        n_req = len(sig) - len(defaults)
        lw.check_arity(line, name, args, n_req, len(sig))
        codes = []
        for i, t in enumerate(sig):
            if i < len(args):
                codes.append(lw.conv(line, args[i], t, "argument %d of %s" % (i + 1, name)).code)
            else:
                codes.append(defaults[i - n_req])
        return T("(%s %s%s)" % (name, obj.code, "".join(" " + c for c in codes)), ret)
    return f


def _overload(*cands):
    """Picks the first candidate whose argument types match exactly
    (after point->connend style conversions)."""
    def f(lw, line, obj, args):
        for match, fn in cands:
            if match(args):
                return fn(lw, line, obj, args)
        lw.err(line, "no matching overload for argument types %s" % [a.ty for a in args])
    return f


def _mutating(name, sig):
    def f(lw, line, obj, args):
        if not obj.lv or obj.lv.startswith("("):
            lw.err(line, "%s on a temporary" % name)
        lw.check_arity(line, name, args, len(sig))
        codes = [lw.conv(line, a, t, "argument of %s" % name).code for a, t in zip(args, sig)]
        return T("(%s %s%s)" % (name, obj.lv, "".join(" " + c for c in codes)), "void")
    return f


def _is(*tys):
    return lambda args: len(args) == len(tys) and all(
        a.ty == t or (t == "double" and a.ty in NUMERIC) for a, t in zip(args, tys))


METHODS = {
    ("router", "processTransaction"): _m("processTransaction", [], "bool"),
    ("router", "setTransactionUse"): _m("setTransactionUse", ["bool"], "void"),
    ("router", "setRoutingParameter"): _m("setRoutingParameter", ["int", "double"], "void", ["-1"]),
    ("router", "setRoutingPenalty"): _m("setRoutingPenalty", ["int", "double"], "void", ["-1"]),
    ("router", "setRoutingOption"): _m("setRoutingOption", ["int", "bool"], "void"),
    ("router", "moveShape"): _overload(
        (_is("shape", "double", "double"), _m("moveShape", ["shape", "double", "double"], "void")),
        (_is("shape", "poly"), _m("moveShapePoly", ["shape", "poly", "bool"], "void", ["false"])),
        (_is("shape", "poly", "bool"), _m("moveShapePoly", ["shape", "poly", "bool"], "void"))),
    ("router", "moveJunction"): _overload(
        (_is("junction", "double", "double"), _m("moveJunction", ["junction", "double", "double"], "void")),
        (_is("junction", "point"), _m("moveJunctionPt", ["junction", "point"], "void"))),
    ("router", "deleteShape"): _m("deleteShape", ["shape"], "void"),
    ("router", "deleteConnector"): _m("deleteConnector", ["conn"], "void"),
    ("router", "deleteJunction"): _m("deleteJunction", ["junction"], "void"),
    ("router", "existsOrthogonalSegmentOverlap"): _m("existsOrthogonalSegmentOverlap", ["bool"], "bool",
                                                     ["false"]),
    ("router", "existsOrthogonalFixedSegmentOverlap"): _m("existsOrthogonalFixedSegmentOverlap", ["bool"],
                                                          "bool", ["false"]),
    ("router", "existsOrthogonalTouchingPaths"): _m("existsOrthogonalTouchingPaths", [], "bool"),
    ("router", "existsCrossings"): _m("existsCrossings", ["bool"], "int", ["false"]),
    ("router", "existsInvalidOrthogonalPaths"): _m("existsInvalidOrthogonalPaths", [], "bool"),
    ("router", "hyperedgeRerouter"): _m("hyperedgeRerouter", [], "rerouter"),
    ("router", "newAndDeletedObjectListsFromHyperedgeImprovement"):
        _m("newAndDeletedObjectListsFromHyperedgeImprovement", [], "ndlists"),
    ("rerouter", "registerHyperedgeForRerouting"): _overload(
        (_is("junction"), _m("registerHyperedgeForReroutingJunction", ["junction"], "int")),
        (_is("connendlist"), _m("registerHyperedgeForReroutingEnds", ["connendlist"], "int"))),
    ("conn", "setSourceEndpoint"): _m("setSourceEndpoint", ["connend"], "void"),
    ("conn", "setDestEndpoint"): _m("setDestEndpoint", ["connend"], "void"),
    ("conn", "setEndpoints"): _m("setEndpoints", ["connend", "connend"], "void"),
    ("conn", "setRoutingType"): _m("setRoutingType", ["int"], "void"),
    ("conn", "setRoutingCheckpoints"): _m("setRoutingCheckpoints", ["checkpoints"], "void"),
    ("conn", "setFixedRoute"): _m("setFixedRoute", ["poly"], "void"),
    ("conn", "clearFixedRoute"): _m("clearFixedRoute", [], "void"),
    ("conn", "makePathInvalid"): _m("makePathInvalid", [], "void"),
    ("conn", "displayRoute"): _m("displayRoute", [], "poly"),
    ("conn", "route"): _m("route", [], "poly"),
    ("conn", "splitAtSegment"): _m("splitAtSegment", ["int"], "jcpair"),
    ("conn", "endpointConnEnds"): _m("endpointConnEnds", [], "endpair"),
    ("conn", "id"): _m("id", [], "int"),
    ("shape", "transformConnectionPinPositions"): _m("transformConnectionPinPositions", ["int"], "void"),
    ("shape", "id"): _m("id", [], "int"),
    ("junction", "setPositionFixed"): _m("setPositionFixed", ["bool"], "void"),
    ("junction", "recommendedPosition"): _m("recommendedPosition", [], "point"),
    ("junction", "position"): _m("position", [], "point"),
    ("junction", "attachedConnectors"): _m("attachedConnectors", [], "connlist"),
    ("junction", "removeJunctionAndMergeConnectors"): _m("removeJunctionAndMergeConnectors", [], "conn"),
    ("junction", "id"): _m("id", [], "int"),
    ("pin", "setExclusive"): _m("setExclusive", ["bool"], "void"),
    ("connend", "shape"): _m("shape", [], "shape"),
    ("connend", "junction"): _m("junction", [], "junction"),
    ("poly", "size"): _m("size", [], "int"),
    ("poly", "setPoint"): _mutating("setPoint", ["int", "point"]),
    ("connlist", "size"): _m("size", [], "int"),
    ("junctionlist", "size"): _m("size", [], "int"),
    ("connlist", "begin"): _m("begin", [], "citer"),
    ("connlist", "end"): _m("end", [], "citer"),
    ("junctionlist", "begin"): _m("begin", [], "jiter"),
    ("junctionlist", "end"): _m("end", [], "jiter"),
    ("checkpoints", "push_back"): _mutating("pushBack", ["checkpoint"]),
    ("connendlist", "push_back"): _mutating("pushBack", ["connend"]),
    ("checkpoints", "size"): _m("size", [], "int"),
}


def _modern_nudge(lw, line, obj, args):
    lw.check_arity(line, "setOrthogonalNudgeDistance", args, 1)
    lw.notes.append("line %s: setOrthogonalNudgeDistance(d) -> setRoutingParameter(idealNudgingDistance, d)"
                    % line)
    return T("(setRoutingParameter %s %d %s)" % (obj.code, ENUMS["idealNudgingDistance"],
                                                 lw.conv(line, args[0], "double").code), "void")


def _modern_addshape(lw, line, obj, args):
    lw.check_arity(line, "addShape", args, 1)
    if args[0].ty != "shape":
        lw.err(line, "addShape of %s" % args[0].ty)
    if not lw.notes or "addShape" not in lw.notes[-1]:
        lw.notes.append("line %s (and following): router->addShape(shape) dropped -- the current "
                        "ShapeRef constructor already adds the shape to the router" % line)
    return T("0", "int", 0)  # discarded


MODERNISE = {
    ("router", "setOrthogonalNudgeDistance"): _modern_nudge,
    ("router", "addShape"): _modern_addshape,
}


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

def convert(name, src_dir):
    path = os.path.join(src_dir, "tests", name + ".cpp")
    with open(path, "rb") as fh:
        raw = fh.read()
    src = raw.decode("utf-8")
    fname = "tests/%s.cpp" % name
    smoke = name in SMOKE_FIXTURES
    code, includes, defines = preprocess(src, fname)
    for inc in includes:
        if inc == '"libcola/cola.h"' and smoke:
            continue
        if inc not in ('"libavoid/libavoid.h"', '"libavoid/connectionpin.h"', "<sstream>", "<cmath>",
                       "<cassert>", "<cstdio>", "<vector>", '"libavoid/assertions.h"'):
            raise ConvertError("%s: unsupported #include %s" % (fname, inc))
    toks = tokenize(code, fname)
    items = Parser(toks, fname).parse_unit()
    lw = Lowerer(name, fname, smoke)
    lw.run(items)
    body = lw.out
    # Earlier outputs overwritten by a later one with the same name cannot
    # be checked against the golden (which holds the last write).
    seen = set()
    for i in range(len(body) - 1, -1, -1):
        m = re.match(r"(\s*)output (\S+) (\S+)$", body[i])
        if m:
            if m.group(3) in seen:
                body[i] = "%soutput-unchecked %s %s" % m.groups()
            seen.add(m.group(3))
    if not smoke:
        missing = []
        for line in body:
            m = re.match(r"\s*(output|delete) (\S+) (\S+)$", line)
            if m and not os.path.exists(os.path.join(GOLDEN_DIR, m.group(3) + ".txt")):
                missing.append(m.group(3))
        if missing:
            raise ConvertError("%s: golden files missing: %s" % (fname, missing))
        if not os.path.exists(os.path.join(GOLDEN_DIR, name + ".exit")):
            raise ConvertError("%s: no golden exit code" % fname)
    sha = hashlib.sha256(raw).hexdigest()
    head = ["# Generated by tools/fixtures/convert.py from upstream libavoid tests/%s.cpp" % name,
            "# (sha256 %s).  Do not edit; re-run the converter." % sha]
    if smoke:
        head.append("# SMOKE TEST: %s upstream (%s), so there is no golden output." % (
            "does not compile", SMOKE_FIXTURES[name]))
        for note in lw.notes:
            head.append("# modernised: " + note)
    head += ["fixture %s" % name, "mode %s" % ("smoke" if smoke else "golden")]
    return "\n".join(head + body) + "\n"


def all_fixture_names(src_dir):
    return sorted(f[:-4] for f in os.listdir(os.path.join(src_dir, "tests")) if f.endswith(".cpp"))


# ---------------------------------------------------------------------------
# C++ re-emission (round-trip verification)
# ---------------------------------------------------------------------------

def ops_tokenize(line):
    toks = []
    i = 0
    while i < len(line):
        c = line[i]
        if c.isspace():
            i += 1
        elif c in "()":
            toks.append(c)
            i += 1
        elif c == '"':
            j = i + 1
            while line[j] != '"':
                j += 2 if line[j] == "\\" else 1
            toks.append(line[i:j + 1])
            i = j + 1
        else:
            j = i
            while j < len(line) and not line[j].isspace() and line[j] not in "()":
                j += 1
            toks.append(line[i:j])
            i = j
    return toks


def ops_parse_expr(toks, pos):
    t = toks[pos]
    if t == "(":
        items = []
        pos += 1
        while toks[pos] != ")":
            e, pos = ops_parse_expr(toks, pos)
            items.append(e)
        return tuple(items), pos + 1
    return t, pos + 1


CPP_TYPES = {
    "router": "Avoid::Router *", "poly": "Avoid::Polygon", "point": "Avoid::Point",
    "connend": "Avoid::ConnEnd", "conn": "Avoid::ConnRef *", "shape": "Avoid::ShapeRef *",
    "junction": "Avoid::JunctionRef *", "pin": "Avoid::ShapeConnectionPin *",
    "checkpoint": "Avoid::Checkpoint", "checkpoints": "std::vector<Avoid::Checkpoint>",
    "connendlist": "Avoid::ConnEndList", "rerouter": "Avoid::HyperedgeRerouter *",
    "ndlists": "Avoid::HyperedgeNewAndDeletedObjectLists",
    "jiter": "Avoid::JunctionRefList::const_iterator", "citer": "Avoid::ConnRefList::const_iterator",
    "jcpair": "std::pair<Avoid::JunctionRef *, Avoid::ConnRef *>",
    "endpair": "std::pair<Avoid::ConnEnd, Avoid::ConnEnd>", "connlist": "Avoid::ConnRefList",
    "int": "int", "double": "double", "bool": "bool",
}

CPP_METHOD = {  # receiver->method(args)
    "processTransaction", "setTransactionUse", "deleteShape", "deleteConnector", "deleteJunction",
    "existsOrthogonalSegmentOverlap", "existsOrthogonalFixedSegmentOverlap",
    "existsOrthogonalTouchingPaths", "existsCrossings", "existsInvalidOrthogonalPaths",
    "hyperedgeRerouter", "newAndDeletedObjectListsFromHyperedgeImprovement", "setSourceEndpoint",
    "setDestEndpoint", "setEndpoints", "setRoutingCheckpoints", "setFixedRoute", "clearFixedRoute",
    "makePathInvalid", "displayRoute", "route", "splitAtSegment", "endpointConnEnds", "id",
    "setPositionFixed", "recommendedPosition", "position", "attachedConnectors",
    "removeJunctionAndMergeConnectors", "setExclusive",
}


EXPR_TYPES = {
    "Router": "router", "ShapeRef": "shape", "JunctionRef": "junction", "ConnRef": "conn",
    "ConnRefEnds": "conn", "Pin": "pin", "PinOld": "pin", "PinJunction": "pin",
    "hyperedgeRerouter": "rerouter", "removeJunctionAndMergeConnectors": "conn", "shape": "shape",
    "junction": "junction", "displayRoute": "poly", "route": "poly", "Polygon": "poly",
    "Rectangle": "poly", "Point": "point", "ps": "point", "recommendedPosition": "point",
    "position": "point", "ConnEnd": "connend", "ConnEndPt1": "connend", "ConnEndPt": "connend",
    "ConnEndShape": "connend", "ConnEndJunction": "connend", "splitAtSegment": "jcpair",
    "endpointConnEnds": "endpair", "newAndDeletedObjectListsFromHyperedgeImprovement": "ndlists",
    "attachedConnectors": "connlist", "newConnectorList": "connlist", "newJunctionList": "junctionlist",
    "deletedConnectorList": "connlist", "deletedJunctionList": "junctionlist",
    "changedConnectorList": "connlist",
}


def etype(e, types):
    """Op type of an op expression (only what the C++ emitter needs)."""
    if isinstance(e, str):
        return types.get(e)
    f = e[0]
    if f in ("first", "second"):
        pair = etype(e[1], types)
        if pair == "jcpair":
            return "junction" if f == "first" else "conn"
        if pair == "endpair":
            return "connend"
        raise ConvertError("emit-cpp: %s of %s" % (f, pair))
    if f == "deref":
        return {"jiter": "junction", "citer": "conn"}[etype(e[1], types)]
    if f == "next":
        return etype(e[1], types)
    if f in ("begin", "end"):
        return {"junctionlist": "jiter", "connlist": "citer"}[etype(e[1], types)]
    return EXPR_TYPES.get(f)


def cpp_expr(e, types):
    if isinstance(e, str):
        if e == "null":
            return "nullptr"
        return e
    f = e[0]
    a = [cpp_expr(x, types) for x in e[1:]]

    def recv(x):
        # pointers use ->, values use .
        if etype(x, types) in ("router", "conn", "shape", "junction", "pin", "rerouter"):
            return ("%s->" if isinstance(x, str) else "(%s)->") % cpp_expr(x, types)
        return ("%s." if isinstance(x, str) else "(%s).") % cpp_expr(x, types)

    binops = {"+", "-", "*", "/", "%", "|", "&", "==", "!=", "<", ">", "<=", ">="}
    if f in binops:
        return "(%s %s %s)" % (a[0], f, a[1])
    if f == "and":
        return "(%s && %s)" % (a[0], a[1])
    if f == "or":
        return "(%s || %s)" % (a[0], a[1])
    if f == "not":
        return "(!%s)" % a[0]
    if f == "neg":
        return "(-%s)" % a[0]
    if f == "?":
        return "(%s ? %s : %s)" % tuple(a)
    if f in ("int", "double", "bool"):
        return "((%s) %s)" % (f, a[0])
    if f == "fabs":
        return "fabs(%s)" % a[0]
    if f == "Router":
        return "new Avoid::Router(%s)" % a[0]
    if f == "Point":
        return "Avoid::Point(%s, %s)" % tuple(a)
    if f == "Polygon":
        return "Avoid::Polygon(%s)" % a[0]
    if f == "Rectangle":
        return "Avoid::Rectangle(%s, %s)" % tuple(a)
    if f == "ConnEnd":
        return "Avoid::ConnEnd()"
    if f == "ConnEndPt1":
        return "Avoid::ConnEnd(%s)" % a[0]
    if f == "ConnEndPt":
        return "Avoid::ConnEnd(%s, %s)" % tuple(a)
    if f == "ConnEndShape":
        return "Avoid::ConnEnd(%s, %s)" % tuple(a)
    if f == "ConnEndJunction":
        return "Avoid::ConnEnd(%s)" % a[0]
    if f == "Checkpoint":
        return "Avoid::Checkpoint(%s)" % a[0]
    if f == "CheckpointDirs":
        return "Avoid::Checkpoint(%s, %s, %s)" % tuple(a)
    if f == "Checkpoints":
        return "std::vector<Avoid::Checkpoint>(%s)" % a[0]
    if f == "ShapeRef":
        return "new Avoid::ShapeRef(%s, %s, %s)" % tuple(a)
    if f == "JunctionRef":
        return "new Avoid::JunctionRef(%s, %s, %s)" % tuple(a)
    if f == "ConnRef":
        return "new Avoid::ConnRef(%s, %s)" % tuple(a)
    if f == "ConnRefEnds":
        return "new Avoid::ConnRef(%s, %s, %s, %s)" % tuple(a)
    if f == "Pin":
        return "new Avoid::ShapeConnectionPin(%s, %s, %s, %s, %s, %s, %s)" % tuple(a)
    if f == "PinOld":
        return "new Avoid::ShapeConnectionPin(%s, %s, %s, %s, %s, %s)" % tuple(a)
    if f == "PinJunction":
        return "new Avoid::ShapeConnectionPin(%s, %s, %s)" % tuple(a)
    if f == "setRoutingParameter" or f == "setRoutingPenalty":
        return "%s%s((Avoid::RoutingParameter) %s, %s)" % (recv(e[1]), f, a[1], a[2])
    if f == "setRoutingOption":
        return "%ssetRoutingOption((Avoid::RoutingOption) %s, %s)" % (recv(e[1]), a[1], a[2])
    if f == "setRoutingType":
        return "%ssetRoutingType((Avoid::ConnType) %s)" % (recv(e[1]), a[1])
    if f == "transformConnectionPinPositions":
        return "%stransformConnectionPinPositions((Avoid::ShapeTransformationType) %s)" % (recv(e[1]), a[1])
    if f == "moveShape" or f == "moveShapePoly":
        return "%smoveShape(%s)" % (recv(e[1]), ", ".join(a[1:]))
    if f == "moveJunction" or f == "moveJunctionPt":
        return "%smoveJunction(%s)" % (recv(e[1]), ", ".join(a[1:]))
    if f in ("registerHyperedgeForReroutingJunction", "registerHyperedgeForReroutingEnds"):
        return "%sregisterHyperedgeForRerouting(%s)" % (recv(e[1]), a[1])
    if f in CPP_METHOD:
        return "%s%s(%s)" % (recv(e[1]), f, ", ".join(a[1:]))
    if f in ("size", "begin", "end", "shape", "junction"):
        return "%s%s()" % (recv(e[1]), f)
    if f == "ps":
        return "%sps[%s]" % (recv(e[1]), a[1])
    if f == "at":
        return "%s[%s]" % (a[0], a[1])
    if f in ("x", "y", "first", "second", "_id", "newJunctionList", "newConnectorList",
             "deletedJunctionList", "deletedConnectorList", "changedConnectorList"):
        return "%s%s" % (recv(e[1]), f)
    if f == "deref":
        return "(*%s)" % a[0]
    if f == "next":
        return "std::next(%s)" % a[0]
    if f == "setPoint":
        return "%s.setPoint(%s, %s)" % (a[0], a[1], a[2])
    if f == "resize":
        return "%s.ps.resize(%s)" % (a[0], a[1])
    if f == "pushBack":
        return "%s.push_back(%s)" % (a[0], a[1])
    raise ConvertError("emit-cpp: unknown op function %s" % f)


def emit_cpp(ops_text):
    out = []
    types = {}
    fixture = None
    mode = None
    depth = 1
    callbacks_used = set()
    for lineno, line in enumerate(ops_text.split("\n"), 1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        toks = ops_tokenize(s)
        op = toks[0]
        ind = "    " * depth

        def ex(i):
            e, p = ops_parse_expr(toks, i)
            if p != len(toks):
                raise ConvertError("emit-cpp: line %d: trailing tokens" % lineno)
            return cpp_expr(e, types)

        if op == "fixture":
            fixture = toks[1]
        elif op == "mode":
            mode = toks[1]
        elif op == "let":
            types[toks[1]] = toks[2]
            out.append("%s%s %s = %s;" % (ind, CPP_TYPES[toks[2]], toks[1], ex(3)))
        elif op == "decl":
            types[toks[1]] = toks[2]
            out.append("%s%s %s;" % (ind, CPP_TYPES[toks[2]], toks[1]))
        elif op == "set":
            lhs, p = ops_parse_expr(toks, 1)
            rhs, p2 = ops_parse_expr(toks, p)
            if p2 != len(toks):
                raise ConvertError("emit-cpp: line %d: trailing tokens" % lineno)
            out.append("%s%s = %s;" % (ind, cpp_expr(lhs, types), cpp_expr(rhs, types)))
        elif op == "do":
            out.append("%s%s;" % (ind, ex(1)))
        elif op in ("output", "output-unchecked"):
            out.append('%s%s->outputDiagram("output/%s");' % (ind, toks[1], toks[2]))
        elif op == "delete":
            out.append("%sdelete %s;" % (ind, toks[1]))
        elif op == "callback":
            callbacks_used.add(toks[2])
            out.append("%s%s->setCallback(cb_%s, %s);" % (ind, toks[1], toks[2], toks[1]))
        elif op == "print":
            out.append("%sprintf(%s);" % (ind, toks[1]))
        elif op == "assert":
            out.append("%sCOLA_ASSERT(%s);" % (ind, ex(1)))
        elif op == "if":
            out.append("%sif (%s) {" % (ind, ex(1)))
            depth += 1
        elif op == "else":
            out.append("%s} else {" % ("    " * (depth - 1)))
        elif op == "end":
            depth -= 1
            out.append("%s}" % ("    " * depth))
        elif op == "ret":
            out.append("%sreturn (int) (%s);" % (ind, ex(1)))
        else:
            raise ConvertError("emit-cpp: line %d: unknown op %s" % (lineno, op))
    head = ["// Regenerated from tests/fixtures/%s.ops by tools/fixtures/convert.py --emit-cpp" % fixture,
            "// (mode %s).  Round-trip check of the op-script conversion." % mode,
            '#include "libavoid/libavoid.h"', "#include <cmath>", "#include <cstdio>", "#include <iterator>",
            "#include <utility>", "#include <vector>", ""]
    for kind in sorted(callbacks_used):
        head += ["static void cb_%s(void *ptr)" % kind, "{",
                 "    Avoid::ConnRef *connRef = (Avoid::ConnRef *) ptr;",
                 '    printf("Connector %u needs rerouting!\\n", connRef->id());',
                 "    const Avoid::PolyLine& route = connRef->%s();" % kind,
                 '    printf("New path: ");',
                 "    for (size_t i = 0; i < route.ps.size(); ++i)",
                 "    {",
                 '        printf("%s(%f, %f)", (i > 0) ? "-" : "", route.ps[i].x, route.ps[i].y);',
                 "    }",
                 '    printf("\\n");', "}", ""]
    return "\n".join(head + ["int main(void)", "{"] + out + ["}", ""])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="verify committed .ops files are up to date")
    ap.add_argument("--emit-cpp", nargs=2, metavar=("OPS", "OUT"), help="regenerate C++ from an op script")
    ap.add_argument("--list", action="store_true", help="list upstream fixture names")
    ap.add_argument("names", nargs="*")
    args = ap.parse_args()
    if args.emit_cpp:
        with open(args.emit_cpp[0]) as fh:
            text = fh.read()
        try:
            cpp = emit_cpp(text)
        except ConvertError as e:
            sys.exit("error: %s" % e)
        with open(args.emit_cpp[1], "w") as fh:
            fh.write(cpp)
        return
    src_dir = upstream_dir()
    names = args.names or all_fixture_names(src_dir)
    if args.list:
        print("\n".join(names))
        return
    os.makedirs(OPS_DIR, exist_ok=True)
    failed = []
    stale = []
    for name in names:
        try:
            text = convert(name, src_dir)
        except ConvertError as e:
            print("FAIL %s: %s" % (name, e), file=sys.stderr)
            failed.append(name)
            continue
        out = os.path.join(OPS_DIR, name + ".ops")
        if args.check:
            try:
                with open(out) as fh:
                    if fh.read() != text:
                        stale.append(name)
            except FileNotFoundError:
                stale.append(name)
        else:
            with open(out, "w") as fh:
                fh.write(text)
    if stale:
        print("stale op scripts (re-run tools/fixtures/convert.py): %s" % " ".join(stale), file=sys.stderr)
    if failed or stale:
        sys.exit(1)
    print("%s %d fixture(s)" % ("checked" if args.check else "converted", len(names)))


if __name__ == "__main__":
    main()
