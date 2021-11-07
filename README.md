# tjs-potfix README

## support postfix
- [x] .if	if (expr)
- [x] .for	for (let i = 0; i < expr.Length; i++)
- [x] .forof	for (let item of expr)
- [x] .foreach	expr.forEach(item => )
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

## feature
1. postfix
![postfix](https://raw.githubusercontent.com/IWANABETHATGUY/tjs-postfix/master/assets/postfix.gif)
2. codeAction
![codeAction](https://raw.githubusercontent.com/IWANABETHATGUY/tjs-postfix/master/assets/codeAction.gif)
3. extract-component
![extract-component](https://github.com/IWANABETHATGUY/tjs-postfix/blob/master/assets/extract-component.gif?raw=true)
4. component-symbol
![component-symbol](https://github.com/IWANABETHATGUY/tjs-postfix/blob/master/assets/component-symbol.gif?raw=true)
5. scss/css completion and jump to definition
![scss-enhancement](https://github.com/IWANABETHATGUY/tjs-postfix/blob/master/assets/scss-completion-jump.gif?raw=true)
