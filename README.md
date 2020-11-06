# tjs-potfix README

## TODO
- [x] .if	if (expr)
- [ ] .else	if (!expr)
- [ ] .null	if (expr === null)
- [ ] .notnull	if (expr !== null)
- [ ] .undefined	if (expr === undefined) or if (typeof expr === "undefined") (see settings)
- [ ] .notundefined	if (expr !== undefined) or if (typeof expr !== "undefined") (see settings)
- [ ] .for	for (let i = 0; i < expr.Length; i++)
- [ ] .forof	for (let item of expr)
- [ ] .foreach	expr.forEach(item => )
- [x] .not	!expr
- [x] .return	return expr
- [x] .var	var name = expr
- [x] .let	let name = expr
- [x] .const	const name = expr
- [x] .log	console.log(expr)
- [x] .error	console.error(expr)
- [x] .warn	console.warn(expr)
- [x] .cast	(\<SomeType\>expr)
- [x] .castas	(expr as SomeType)
- [x] .new	new expr()