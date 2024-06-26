#pragma once
#include "Framework.h"

class Zone;

class ZoneLink
{
public:

	Zone* Target;
	long TargetId;
	ZoneLink* Next;
	double From;
	double To;
	long SourceFromPixel;
	long SourceToPixel;
	long TargetFromPixel;
	long TargetToPixel;

	double BorderResistance;
	long BorderResistancePixel;

	//long SourceLengthPixel;
	//long TargetLengthPixel;
	[[nodiscard]] Zone* At(const double pos) const;
	const ZoneLink* AtPhysical(double pos) const;
	[[nodiscard]] const ZoneLink* AtPixel(long pos) const;
	[[nodiscard]] long ToTargetPixel(long v) const;
	ZoneLink(const double from, const double to, const long sourceFromPixel, const long sourceToPixel, const long targetFromPixel, const long targetToPixel, const double borderResistance, const long targetId = -1);
	~ZoneLink() {delete Next;}
};

